# A/B stackful de Routine con cachés por Processor

## Alcance

Esta ronda compara el pool global Go-inspired con `ProcessorStackCache` reales,
uno creado y poseído por cada worker después de enlazarse a su `Processor`.
Cada `Routine` se crea en ese worker, se publica, ejecuta la misma closure en
su stack real mediante `run_once`, y devuelve el lease a la caché del mismo
Processor antes de descartarse. El baseline hace el mismo ciclo sobre los
mismos `Processor` y workers, pero adquiere del pool global y deja que el lease
retorne al pool al descartar la Routine.

El benchmark ejecuta directamente las rutinas desde sus workers enlazados; no
pasa por `MachineManager`, las colas de dispatch, injection, stealing ni
`RoutineJoinHandle`. El join al final de cada proceso es el join de los threads
worker. Esta exclusión es intencional: compara el efecto de la política de
asignación y retorno en un ciclo stackful idéntico, sin afirmar que ya mida el
scheduler end-to-end. Por ello, es evidencia de la penalidad en una ruta
Routine/Processor controlada, no atribución causal completa de los benchmarks
de `MachineManager`.

## Método reproducible

Comando:

```text
cargo +1.85.0 bench --bench routine_processor_cache_ab
```

El proceso coordinador lanza hijos separados y alterna el orden pool global /
cachés por cantidad de workers. Se ejecutaron tres matrices completas; cada
matriz contiene un hijo aislado por modo y cantidad de workers. Cada hijo
ejecuta 1, 2, 4, 8, 12 o 16 workers, 10 muestras de 256 rutinas por workload,
y las cargas no-op, CPU64 y CPU4096. En total son 30 muestras por celda y
1.080 lotes cronometrados.
Los workers enlazan con un `Processor` real; cada cache path tiene un cache
único por Processor y su owner nunca cambia. Se usan stacks solicitados de
64 KiB, spans de 64 slots y un target de caché de 1 MiB. Ambos modos usan la
misma implementación Go-inspired, configuración, número de operaciones y
secuencia de `Routine::new`/`publish`/`run_once`/finalización.

Se calientan 64 rutinas por worker y por workload antes de las muestras.
Barreras sincronizan inicio y fin de cada lote; su coste forma parte del tiempo
de lote en ambos modos. El coordinador mide el tiempo de pared de cada lote;
no hay timers por rutina ni contadores o métricas intrusivas dentro de la ruta
cronometrada. La medición es end-to-end para este ciclo directo de Routine en
workers, no para envío desde un caller ni dispatch de `MachineManager`.

Entorno: Rust 1.85.0 (`4d91de4e4`), Linux x86_64, kernel
`7.0.0-42-generic`, Intel Core i5-12450H, 12 CPUs lógicas y governor
`powersave`. No se fijó afinidad, y no se controló carga externa del host. Los
datos completos están en los logs de cada matriz:
[`primera`](./runs/20261115-routine-processor-cache-ab-uninstrumented.log),
[`segunda`](./runs/20261115-routine-processor-cache-ab-repeat2.log) y
[`tercera`](./runs/20261115-routine-processor-cache-ab-repeat3.log). Cada log
incluye las diez muestras y rangos por configuración.

## Resultados

Throughput mediano de 256 ejecuciones por lote, en millones de rutinas/s,
agregado sobre las 30 muestras:

| Workers | No-op global | No-op cache | CPU64 global | CPU64 cache | CPU4096 global | CPU4096 cache |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1,956 | 2,112 | 1,456 | 1,746 | 0,094 | 0,104 |
| 2 | 2,363 | 2,556 | 2,148 | 2,518 | 0,197 | 0,191 |
| 4 | 1,695 | 3,597 | 1,734 | 3,213 | 0,268 | 0,257 |
| 8 | 1,445 | 3,179 | 1,456 | 3,251 | 0,486 | 0,482 |
| 12 | 1,260 | 2,534 | 1,223 | 2,647 | 0,522 | 0,425 |
| 16 | 1,097 | 2,115 | 1,094 | 2,105 | 0,409 | 0,433 |

El contraste de caché/global fue:

| Workers | No-op | CPU64 | CPU4096 |
|---:|---:|---:|---:|
| 1 | 1,08x | 1,20x | 1,11x |
| 2 | 1,08x | 1,17x | 0,97x |
| 4 | 2,12x | 1,85x | 0,96x |
| 8 | 2,20x | 2,23x | 0,99x |
| 12 | 2,01x | 2,16x | 0,81x |
| 16 | 1,93x | 1,92x | 1,06x |

Con 4 o más workers, la caché mejora de forma consistente no-op y CPU64 en
estas tres matrices; el throughput mediano llega a aproximadamente 2,2x en
workers 8 y 12. Con 1 y 2 workers la mejora es pequeña y las muestras de dos
modos se solapan ampliamente. En CPU4096 no se observa ganancia consistente:
los resultados están cerca de paridad y la caché queda por debajo del global
a 2, 4, 8 y 12 workers. Es coherente con que la mejora del cache path sea más
visible cuando el trabajo útil es pequeño.

La dispersión sigue siendo importante. Por ejemplo, el lote no-op global de
8 workers tuvo un rango agregado de 165–3.120 µs frente a 74–97 µs con cachés;
también aparecen outliers en otros lotes concurrentes. Son 30 observaciones,
no intervalos de confianza ni p99 robusto. Las razones describen este host y
estos lotes, y no demuestran escalabilidad general. El throughput no mejora
monótonamente al aumentar workers en ninguno de los modos.

## Comprobaciones de cache, lifecycle y memoria

Los 36 procesos de modo/workers/matriz reportaron exactly-once, cero fallos, cero
leases activos y asignaciones igualadas con liberaciones. La ruta global no
registró actividad de caché. En la ruta local, los misses/refills fueron
exactamente 1 por Processor: 1, 2, 4, 8, 12 o 16, respectivamente; las 7.871–
10.736 operaciones restantes por proceso fueron hits. Al terminar, las cachés se
vaciaron, `trim_to_retained_spans(0)` dejó cero memoria virtual reservada y
cero páginas accesibles retenidas.

La comprobación funcional del constructor/release confirma que una Routine no
puede devolver su stack antes de estar terminal y que, una vez completada, el
lease puede volver a la caché antes de descartar el handle. Si falla el retorno,
el lease permanece en la Routine y su destrucción conserva el fallback seguro
al pool global.

## Conclusión y siguiente paso causal

Hay evidencia stackful de que cachés reales, exclusivas por Processor, reducen
el coste del ciclo directo de Routine para no-op y CPU64 a partir de cuatro
workers. Para CPU4096 no se observa ganancia estable. Esto localiza una
penalidad del pool global en esta ruta Routine/Processor con trabajo corto,
pero no prueba que sea la causa dominante del coste del scheduler
`MachineManager`.

El siguiente experimento para atribución completa debe preservar estas cachés
en los workers reales de `MachineManager` y comparar el mismo workload con
dispatch, publicación y join habilitados. Para eso el runtime necesita una ruta
de asignación de stack ejecutada por el worker elegido (o un protocolo explícito
de handoff) y retorno al Processor propietario; no se debe disfrazar la
asignación caller-side del benchmark actual como local al Processor. La
integración de producción debe esperar a esa comparación y a revisar la
retención bajo carga y shutdown.
