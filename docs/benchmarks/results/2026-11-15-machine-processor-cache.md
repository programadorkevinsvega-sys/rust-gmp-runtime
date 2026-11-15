# A/B del scheduler con caché Go-inspired por Processor

## Pregunta y alcance

¿La asignación y reutilización del stack en el worker que ejecuta la rutina
mejora el camino completo del scheduler en más de 20% frente al pool global?
La comparación usa el mismo `MachineManager` y `dispatch_new` en ambos modos.
En los dos, el worker seleccionado crea la `Routine`, reserva su transición,
ejecuta la closure en el stack real, finaliza y resuelve el join. Cambia solo
la política de leases: pool global, o caché Go-inspired exclusiva del worker
asociado al Processor ejecutor.

No se comparan dos schedulers ni se aísla el allocator del costo de dispatch.
La medición incluye submit, creación en worker, cola/selección, context switch,
ejecución, finalización, liberación del lease y join; por tanto, un resultado no
atribuye causalidad a un componente individual.

## Método reproducible

Comando:

```text
cargo +1.85.0 bench --bench machine_processor_cache_ab
```

Cada modo y cantidad de workers se ejecutó en un proceso hijo aislado. Se
hicieron tres matrices con 1, 2, 4, 8, 12 y 16 workers; el orden global/cache
se invirtió en la matriz 2. Cada celda contiene 30 muestras por matriz, 90 en
total; cada muestra despacha y une 256 rutinas. Se midieron no-op, CPU64 y
CPU4096. Se calentaron 64 tareas de cada workload antes de medir. Los stacks
solicitados son de 64 KiB, los spans de 64 slots y el objetivo de caché es 1
MiB por clase.

Las closures usan exactamente la misma función CPU/xorshift y la misma
secuencia de semillas por lote que el benchmark directo
[`routine_processor_cache_ab`](./2026-11-15-routine-processor-cache.md); se
comparó también el código de ambas definiciones de workload.

El timer de pared del caller cubre el lote `dispatch_new` más el join de todos
los handles. No hay timers ni contadores añadidos por tarea en ese intervalo;
el accounting normal del allocator permanece activo e igual para ambos modos.
Una tanda separada de 128 rutinas por proceso comprueba exactly-once. Las
métricas del allocator verifican además que las 23.360 asignaciones por proceso
se correspondan con 23.360 liberaciones, sin stacks activos, errores ni slots
locales retenidos después de shutdown.

Para cada celda, la razón de throughput `caché/global` es la mediana del tiempo
global dividida por la mediana del tiempo cache; el número de operaciones es
idéntico. Los intervalos son percentiles 2,5–97,5 de 30.000 remuestreos
bootstrap estratificados por las tres matrices: se remuestrean matrices y luego
muestras dentro de cada matriz y modo. El seed del análisis fue `74921`.
Con solo tres matrices, estos intervalos son evidencia descriptiva, no una
garantía estadística para otros hosts o cargas.

Entorno: Rust 1.85.0, Linux x86_64, kernel `7.0.0-42-generic`, Intel Core
i5-12450H (12 CPUs lógicas), governor `powersave`. No se fijó afinidad ni se
controló carga externa del host.

## Resultados

Los tiempos son medianas de lote en µs; una razón mayor que 1 favorece la
caché. La columna `IC 95%` es el bootstrap estratificado descrito arriba.

| Workers | Carga | Global (µs) | Caché (µs) | Throughput caché/global | IC 95% |
|---:|---|---:|---:|---:|---:|
| 1 | no-op | 309,233 | 304,454 | 1,016x | [0,916; 1,224] |
| 1 | CPU64 | 340,930 | 338,435 | 1,007x | [0,925; 1,369] |
| 1 | CPU4096 | 2.670,860 | 2.641,234 | 1,011x | [0,993; 1,071] |
| 2 | no-op | 293,882 | 259,570 | 1,132x | [1,088; 1,200] |
| 2 | CPU64 | 258,426 | 253,928 | 1,018x | [0,875; 1,235] |
| 2 | CPU4096 | 1.319,463 | 1.314,846 | 1,004x | [0,989; 1,035] |
| 4 | no-op | 383,380 | 362,055 | 1,059x | [0,906; 1,232] |
| 4 | CPU64 | 331,112 | 352,647 | 0,939x | [0,815; 0,998] |
| 4 | CPU4096 | 780,188 | 773,417 | 1,009x | [0,958; 1,075] |
| 8 | no-op | 571,805 | 666,774 | 0,858x | [0,766; 0,931] |
| 8 | CPU64 | 562,793 | 611,630 | 0,920x | [0,773; 0,997] |
| 8 | CPU4096 | 467,459 | 468,443 | 0,998x | [0,938; 1,023] |
| 12 | no-op | 748,299 | 768,474 | 0,974x | [0,935; 1,046] |
| 12 | CPU64 | 708,523 | 714,889 | 0,991x | [0,946; 1,022] |
| 12 | CPU4096 | 419,203 | 436,998 | 0,959x | [0,903; 1,040] |
| 16 | no-op | 845,630 | 846,053 | 0,999x | [0,977; 1,031] |
| 16 | CPU64 | 806,509 | 823,557 | 0,979x | [0,946; 1,017] |
| 16 | CPU4096 | 468,002 | 471,237 | 0,993x | [0,921; 1,056] |

**No se demuestra una mejora superior al 20%.** Ninguna celda tiene un límite
inferior del intervalo por encima de `1,20x`. La mejor mediana es 1,132x
(no-op, 2 workers), y su límite superior redondeado es 1,200x, no evidencia de
una mejora mayor al 20%. Con 8 workers, la caché fue más lenta en no-op y CPU64
(0,858x y 0,920x); los efectos también cambian entre matrices. No hay un
beneficio estable que justifique cambiar el default de producción.

Esto difiere del benchmark directo de `Routine` con workers enlazados a P, que
había mostrado ventajas en trabajo corto. Al usar ahora el mismo workload y
añadir creación, publicación, selección y join reales del `MachineManager`, esa
ventaja no se traduce en una mejora reproducible del throughput end-to-end en
esta máquina. Los resultados no prueban que el scheduler sea el cuello de
botella: muestran que la caché por sí sola no mejora de forma estable este
camino completo.

## Validación de leases y conclusión operativa

Los 36 procesos reportaron exactly-once en la tanda de validación, cero
asignaciones/liberaciones desbalanceadas, cero fallos, cero stacks activos y
cero slots locales retenidos después del shutdown. En la ruta cache, se
registraron entre 1 y 16 misses iniciales/refills según workers y más de 23 mil
hits por proceso. El estado de Machine `Preparing` incluye el tiempo de
creación/adquisición y se diferencia del estado `Running`.

La ruta `start_with_processor_caches` queda disponible como opción explícita
para más experimentos, pero **no se recomienda como default** con esta
evidencia. La API existente `MachineManager::start`/`dispatch` conserva su
comportamiento. Para buscar una ganancia >20% se necesita identificar primero
otra penalidad del camino scheduler; no se debe atribuir esta diferencia al
mutex, al stealing o al allocator sin una prueba que los separe.

## Recomendación para el runtime

| Componente | Decisión |
|---|---|
| Fixed Stack | Mantener como default de referencia |
| Go-inspired global pool | Mantener disponible como motor alternativo |
| Go-inspired Processor caches | Mantener opt-in experimental |
| `MachineManager::start` / `dispatch` | No cambiar el comportamiento |
| `start_with_processor_caches` | Conservar para experimentación |
| Assembly context switch | No volver a optimizar sin nueva evidencia |
| Scheduler queues / stealing | No cambiar por estos resultados |

Mantener Fixed como default de referencia no implica que sea necesariamente
el motor más rápido; significa que esta evidencia no justifica cambiar el
comportamiento predeterminado. La integración de Processor caches está
validada funcionalmente, pero no justificada como optimización predeterminada.
El pool Go-inspired conserva su valor como allocator independiente y las
cachés locales muestran ventajas en microbenchmarks controlados, pero esas
ventajas no se traducen en una mejora estable del `MachineManager`.

## Siguiente experimento causal

No se deben añadir contadores diagnósticos al benchmark principal. Una
ejecución diagnóstica independiente debe separar:

1. Submit hasta la aceptación del dispatch.
2. Tiempo en `Preparing`, incluida la adquisición del stack.
3. Tiempo en cola hasta que el worker selecciona la rutina.
4. Ejecución, incluidos los cambios de contexto.
5. Finalización y devolución del lease.
6. Resolución del join.

La atribución debe comparar uno y dos workers con cuatro y ocho. Una vez
localizada una penalidad con instrumentación, cualquier optimización se debe
verificar de nuevo con un benchmark sin instrumentación. El escalado irregular
del host de 12 CPUs lógicas no implica que más workers deban mejorar el
throughput. No hay evidencia en este A/B para modificar el mutex, el scheduler
ni el stealing.

Logs completos:

- [Matriz 1](./runs/20261115-machine-processor-cache-ab.log)
- [Matriz 2, orden invertido](./runs/20261115-machine-processor-cache-ab-repeat2.log)
- [Matriz 3](./runs/20261115-machine-processor-cache-ab-repeat3.log)
