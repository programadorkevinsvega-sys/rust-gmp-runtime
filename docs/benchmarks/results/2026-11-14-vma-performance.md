# Auditoría de rendimiento del VMA preflight

## Alcance y entorno

La auditoría usa el allocator fijo real de F2; no cambia el modelo de stacks.
Linux 7.0.0-42-generic x86_64, Intel Core i5-12450H, 12 CPUs lógicas,
governor `powersave`, Rust 1.85.0, `vm.max_map_count=1.048.576`. El proceso
partió de aproximadamente 32 VMAs. Se conservaron como referencias separadas
el [baseline original de F2](2026-11-14-d9617743aba79bbdf62b3ea63c2a053cce5ef01c.md)
y el [benchmark de control VMA](2026-11-14-vma-control.md); sus datos no se
reescribieron.

## Cuándo se consulta procfs

- Cada `allocate()` toma el mutex de admisión y calcula el presupuesto usando
  el último snapshot y el coste de todas las reservas ocurridas desde que
  comenzó su lectura. No abre `/proc` por stack.
- El primer check consulta `/proc/sys/vm/max_map_count` y `/proc/self/maps`.
  Después, el snapshot se refresca tras 262.144 operaciones de lifecycle
  (asignaciones y liberaciones combinadas) o cuando se llama explícitamente a
  `metrics()`/`vma_snapshot()`. `preflight_additional_stacks()` es advisory y
  puede reutilizar el snapshot mientras no venza esa cadencia.
- Los snapshots se leen fuera del mutex. Durante un refresco periódico, otras
  asignaciones pueden continuar usando el snapshot previo y el estimate interno
  protegido por mutex. El primer snapshot sigue siendo obligatorio; las
  asignaciones esperan a que exista sin retener el mutex. Las consultas
  explícitas de snapshot esperan un refresco exitoso.
- El snapshot ancla un contador monótono de reservas al valor capturado al
  comenzar la lectura, no al del final. Así, toda asignación concurrente se
  suma al snapshot aunque `/proc/self/maps` ya la haya incluido; esto puede
  sobrecontar, nunca perder del accounting interno una reserva realizada
  durante la lectura. Las liberaciones no restan de ese contador hasta que un
  snapshot posterior pueda observarlas. Los mappings externos aún pueden
  cambiar concurrentemente y los syscalls nativos siguen pudiendo fallar.

Antes de optimizar, la cadencia era 65.536 operaciones. El coste fijo del
mutex, el cálculo y la contabilidad no justificaba por sí solo el aumento: una
medición de 10.000 checks de presupuesto dio aproximadamente 1 ns/check; una
medición de allocator a baja cardinalidad dio 2.212 ns/stack con preflight y
2.111 ns sin preflight (+4,8%). La adquisición no contenciosa del mutex dio
12 ns, y un mutex sintético con 8 workers dio 93 ns por adquisición en
promedio. Son microbenchmarks cortos y dependen del host; no son objetivos de
rendimiento.

## Coste de lectura y parseo

Con cerca de 800.000 VMAs (400.000 stacks vivos), 10 repeticiones midieron:

| Operación | Media de 10 muestras | Notas |
|---|---:|---|
| Lectura de `/proc/self/maps` | 111,21 ms | Bytes de procfs, sin parser del runtime. |
| Parseo y validación de snapshot capturado | 86,51 ms | Cerca de 800.040 líneas. |
| Lectura y recuento de regiones | 203,63 ms | Incluye `BufReader` y validación streaming. |
| Snapshot completo (límite + mapas) | 190,60 ms | Media de 10 muestras en ese proceso. |

El orden de lecturas produjo conteos adyacentes de 800.040 y 800.041; las
asignaciones del propio proceso pueden cambiar procfs entre lecturas. El conteo
solo es una observación puntual.

Con alrededor de 200.000 VMAs, la lectura y parseo completos tardaron
aproximadamente 47–49 ms por snapshot. El coste escala con la cantidad de
regiones; no se debe describir como coste constante por consulta.

## Syscalls y lock

Con aproximadamente 800.000 VMAs preexistentes, se reservaron/protegieron/
liberaron 1.000 mappings por muestra en 10 muestras:

| Operación | Mediana por syscall | Rango observado |
|---|---:|---:|
| `mmap` | 738 ns | 712–903 ns |
| `mprotect` | 1.288 ns | 1.251–1.400 ns |
| `munmap` | 1.124 ns | 1.040–1.200 ns |

El microbenchmark agrupa 1.000 llamadas y reporta coste medio por llamada;
no representa una latencia p99. La secuencia de allocator incluye además
validación, accounting y locks, por lo que estos valores no deben sumarse como
si fueran una medición exacta del coste total.

En la implementación anterior al cambio descrito abajo, la lectura completa
(~191 ms) se efectuaba dentro del lock de admisión. El test manual midió para
ese waiter mediana de 192,65 ms y rango 188,81–195,11 ms en 10 muestras. Esas
cifras son el baseline de la contención eliminada, no el comportamiento actual.

El presupuesto usa un coste máximo estimado de dos VMA por stack: una reserva
`mmap(PROT_NONE)` seguida de habilitar sus páginas usables con `mprotect`
produce hasta dos regiones de permisos distintos. No afirma que cada stack
aumente exactamente dos VMA observadas; fusiones con regiones adyacentes pueden
reducir el delta. Es una estimación de admisión derivada de la disposición real
actual, mientras `vma_current` sigue siendo el conteo procfs observado.

## A/B del allocator en alta cardinalidad

Con 400.000 stacks base y lotes adicionales de 100.000 (hasta ~1.000.000 VMA),
10 muestras alternaron preflight habilitado/deshabilitado. Los tiempos incluyen
reserva y accounting, no la liberación del lote:

| Cadencia | Preflight mediana | Sin preflight mediana | Diferencia |
|---|---:|---:|---:|
| 65.536 operaciones (medición inicial) | 642,62 ms | 215,28 ms | +198,5% |
| 262.144 operaciones (cadencia revisada) | 402,37 ms | 206,78 ms | +94,6% |

En esta medición histórica, las operaciones que encontraban el umbral pagaban
un snapshot de ~191 ms mientras conservaban el mutex de admisión. Al pasar a
262.144, se redujeron los refreshes respecto de la cadencia 65.536, pero el
bloqueo global durante el snapshot aún existía en esa versión. La frecuencia
era una heurística; ningún snapshot antiguo se presentaba como capacidad
garantizada. La sección de seguimiento registra la eliminación posterior del
bloqueo global y la reconciliación de la implementación actual.

## Repetición de escala

Comando: `RUST_GMP_RUN_SCALE=1 cargo +1.85.0 bench --bench scale`. El benchmark
calienta 64 stacks, toma 10 muestras y mide reserva/liberación por separado.
Se solicitaron stacks usables de 8 KiB. 1M fue rechazado preventivamente como
en la referencia previa; las escalas 1k–500k no tuvieron fallos nativos.

| Stacks | F2 original: reserva | Preflight 65k: reserva | Cadencia 262k: reserva | Cadencia 262k: liberación |
|---:|---:|---:|---:|---:|
| 1.000 | 1,631 ms | 1,995 ms | 2,158 ms | 1,272 ms |
| 10.000 | 18,981 ms | 19,033 ms | 19,872 ms | 12,234 ms |
| 100.000 | 197,809 ms | 215,984 ms | 188,547 ms | 113,215 ms |
| 500.000 | 922,987 ms | 1.854,244 ms | 1.095,234 ms | 579,120 ms |

Son medianas de diez muestras, no una comparación pareada: el baseline F2 y
las repeticiones tienen workloads/árboles de trabajo y presión del kernel
distintos. En esta repetición anterior a la reconciliación, el resultado de
500k estuvo +18,7% respecto del baseline F2, -40,9% respecto de la repetición
con cadencia 65k y -47,4% respecto del informe VMA inicial (2.080,125 ms). La
referencia inicial reportaba +125,37%; esa cifra no se reproduce en esta
repetición. El A/B controlado identifica una contribución apreciable de
snapshots periódicos, pero no permite atribuir todo el diferencial histórico
a ellos.

El resultado a 500k fue 1.095.233.846 ns (máximo 1.229.892.713 ns; dispersión
193.861.142 ns). La ejecución observó 1.000.031 VMA, 500.000 mappings
conocidos y 1.000.000 VMA estimadas. El preflight de 1M calculó 2.000.000 VMA,
excediendo el límite real, y rechazó sin intentar reservar.

### Repetición completa con reconciliación automática

Comando ejecutado después de incorporar el retry: `RUST_GMP_RUN_SCALE=1
cargo +1.85.0 bench --bench scale`. Se completaron diez muestras por escala
de 1k, 10k, 100k y 500k. La medición informa por separado el preflight
cacheado, la reserva y la liberación; además registra VMA antes, con el lote
activo y después de liberarlo. Los snapshots de métricas ocurren fuera de los
tramos cronometrados de reserva/liberación.

| Stacks | Preflight mediana (ns) | Reserva mediana / máximo / dispersión (ns) | Liberación mediana / máximo / dispersión (ns) | Preflight tras liberación mediana / máximo / dispersión (ns) | VMA con lote activo |
|---:|---:|---:|---:|---:|---:|
| 1.000 | 72 | 1.926.819 / 2.129.033 / 397.459 | 1.261.023 / 1.448.580 / 291.152 | 133 / 269 / 183 | 2.031 |
| 10.000 | 69 | 20.212.120 / 21.955.709 / 2.581.179 | 14.375.977 / 16.360.532 / 2.672.710 | 421 / 1.000 / 729 | 20.031 |
| 100.000 | 132 | 199.757.884 / 224.830.243 / 36.325.316 | 136.702.428 / 150.845.827 / 18.338.868 | 854 / 1.269 / 791 | 200.031 |
| 500.000 | 135 | 1.145.301.788 / 1.385.959.585 / 334.056.680 | 700.261.944 / 745.439.614 / 98.059.611 | 205.639.675 / 242.339.143 / 49.784.390 | 1.000.031 |

En las muestras completadas no hubo fallos nativos ni rechazos. Cada muestra
partió de 32 VMAs observadas y volvió a 32 después de liberar el lote. En
500k se registraron 500.000 mappings conocidos y una estimación de 1.000.000
VMA del runtime.

En cada nivel, los bytes virtuales reservados/accesibles fueron
12.288.000/8.192.000 (1k), 122.880.000/81.920.000 (10k),
1.228.800.000/819.200.000 (100k) y 6.144.000.000/4.096.000.000 (500k).
La RSS aproximada observada mientras el lote estaba activo varió entre
2.490.368–2.531.328, 2.859.008–2.932.736, 6.459.392–6.582.272 y
22.458.368–26.447.872 bytes, respectivamente. Son lecturas de `statm`, no
memoria residente atribuible exactamente a los stacks.

La escala de 1M no reservó mappings. El preflight rechazó después de ejecutar
la reconciliación automática y el segundo check; la llamada completa tardó
16.698 ns. El snapshot fresco observó 32 VMAs; se estimaban 2.000.000 VMA
requeridas, frente al límite de 1.048.576 y margen 1.024. Se registró un
rechazo preventivo y cero fallos nativos. Esta es evidencia del costo de la
ruta de rechazo en este estado de baja cardinalidad, no un benchmark del caso
de recuperación tras liberación a alta cardinalidad.

El probe posterior a cada liberación solicita capacidad para 25.000 stacks.
En 500k, primero libera 100.000 stacks: el snapshot cacheado sigue estimando
aproximadamente 1.000.000 VMA, por lo que el probe se rechaza provisionalmente.
La reconciliación observa cerca de 800.000 VMA, confirma que hay presupuesto y
el segundo check admite el probe. `reconciled_after_release=true` en las diez
muestras de 500k. La medición de reserva/liberación excluye el tiempo del probe.

Datos brutos de preflight, reserva, liberación y probe posterior por muestra
(ns), en orden:

| Stacks | Preflight |
|---:|---|
| 1.000 | 141, 86, 59, 70, 70, 55, 59, 74, 81, 87 |
| 10.000 | 64, 71, 99, 207, 113, 54, 69, 69, 55, 64 |
| 100.000 | 76, 96, 164, 154, 286, 104, 159, 154, 110, 104 |
| 500.000 | 104, 121, 132, 221, 138, 255, 125, 133, 140, 173 |

| Stacks | Reserva |
|---:|---|
| 1.000 | 2.129.033, 2.003.635, 1.950.343, 1.903.249, 1.903.295, 1.781.701, 1.731.574, 1.900.493, 1.983.277, 2.107.901 |
| 10.000 | 19.478.087, 19.550.299, 19.787.288, 21.955.709, 20.832.509, 19.374.530, 21.410.708, 20.094.800, 20.928.101, 20.329.441 |
| 100.000 | 188.504.927, 193.388.166, 224.830.243, 215.550.131, 214.510.719, 203.087.536, 205.942.972, 194.864.474, 196.428.232, 192.540.329 |
| 500.000 | 1.069.105.666, 1.139.137.978, 1.051.902.905, 1.216.986.328, 1.238.672.164, 1.141.375.982, 1.242.367.955, 1.385.959.585, 1.141.193.965, 1.149.227.594 |

| Stacks | Liberación |
|---:|---|
| 1.000 | 1.448.580, 1.205.743, 1.253.356, 1.268.690, 1.376.034, 1.157.952, 1.334.821, 1.157.428, 1.317.160, 1.243.528 |
| 10.000 | 13.819.444, 13.817.987, 14.681.446, 14.148.781, 14.603.173, 15.372.925, 16.360.532, 14.726.217, 13.687.822, 14.033.249 |
| 100.000 | 133.089.266, 137.480.892, 150.845.827, 150.826.902, 136.370.429, 133.125.183, 143.664.589, 133.955.841, 137.034.428, 132.506.959 |
| 500.000 | 647.380.003, 656.322.884, 676.028.010, 715.800.762, 737.152.494, 693.919.469, 745.439.614, 743.716.999, 706.604.419, 680.864.149 |

| Stacks | Probe posterior |
|---:|---|
| 1.000 | 269, 181, 116, 88, 86, 110, 265, 151, 133, 134 |
| 10.000 | 535, 309, 1.000, 372, 402, 313, 494, 486, 271, 441 |
| 100.000 | 791, 1.097, 1.269, 846, 858, 1.230, 1.107, 478, 711, 851 |
| 500.000 | 192.554.753, 201.593.295, 194.462.727, 212.429.836, 242.339.143, 205.807.758, 230.128.433, 223.348.522, 203.368.708, 205.471.592 |

La repetición mide las rutas finales de admisión por stack y la reconciliación
de rechazo del caso de 1M. El allocator de escala es serial; no demuestra
throughput concurrente. Esa brecha se midió después con el benchmark paralelo
de ciclos `allocate`/`Drop` y refresh VMA descrito en el
[informe concurrente](./2026-11-14-allocator-concurrent.md).
La mediana nueva de reserva a 500k es +24,1% frente al baseline F2 de
922,987 ms; las muestras no son pareadas y no permiten atribuir causalidad.

## Cambio implementado y decisión de arquitectura

Se mantuvo la serialización de presupuesto y syscalls. El coste VMA monótono de
las reservas se actualiza dentro del estado protegido por el mutex de admisión,
evitando tomar el mutex de métricas adicional en cada `allocate()`. La
frecuencia máxima de snapshot pasó de 65.536 a 262.144 operaciones. Consultas
explícitas de métricas siguen obteniendo una observación fresca. Los fallos
nativos y de limpieza parcial siguen propagándose; no se trata el cache como
garantía.

### Seguimiento: snapshot fuera del mutex de admisión

Se separó la lectura de procfs de la sección crítica. Solo el hilo que inicia
un refresco hace la lectura; las asignaciones concurrentes usan el snapshot
anterior y actualizan/reservan el contador de reservas bajo el mutex. Si aún no existe un
snapshot, los threads esperan mediante `Condvar`, que libera el mutex mientras
esperan. Los lectores explícitos coalescen con el refresco en curso o inician
uno nuevo. Los errores del lector se propagan al solicitante; nunca se
convierten en cero VMA o capacidad ilimitada.

Comando: `cargo +1.85.0 test --lib high_vma_preflight_costs -- --ignored
--nocapture`. Con cerca de 800.000 VMA, 10 muestras encontraron snapshots de
244,40–292,87 ms (mediana 256,17 ms), mientras una asignación concurrente
completa midió 13.265–23.054 ns (mediana 15.850 ns). La prueba unitaria bloquea
deliberadamente el lector y verifica que `allocate()` termina antes de
liberarlo; la medición de alta cardinalidad confirma el mismo camino con
procfs real. El baseline de espera del mutex era ~192,65 ms.

La lectura aún retrasa al thread que inició el refresco; el cambio elimina la
pausa global de admisión, no el coste CPU/I/O del snapshot. En esta repetición,
lotes de 100.000 stacks variaron entre 203,89 y 277,79 ms con preflight
(mediana 228,46 ms), frente a 198,20–262,80 ms sin preflight (mediana
213,55 ms). Las muestras no son pareadas y dependen de la presión del entorno;
queda coste de throughput, aunque otros allocators ya pueden avanzar.

### Implementación actual: reconciliación después de rechazo

Ante un rechazo con un snapshot potencialmente inflado, el allocator ahora
libera el mutex, inicia o comparte un refresh, y repite la admisión una vez.
Las liberaciones durante el refresh no decrementan el contador monótono; el
snapshot nuevo observa los mappings ya liberados y vuelve a anclar el contador.
Así se mantiene el criterio conservador ante carreras sin conservar
indefinidamente la estimación anterior.

Datos brutos del refresco y una asignación concurrente (ns), repetición de
componentes VMA anterior a la reconciliación automática:

| Muestra | Snapshot | `allocate()` concurrente |
|---:|---:|---:|
| 0 | 255.332.759 | 18.176 |
| 1 | 292.870.770 | 16.332 |
| 2 | 246.004.301 | 13.850 |
| 3 | 247.879.768 | 23.054 |
| 4 | 261.577.869 | 15.367 |
| 5 | 268.387.424 | 13.684 |
| 6 | 244.400.013 | 19.279 |
| 7 | 257.007.700 | 15.221 |
| 8 | 263.591.985 | 13.265 |
| 9 | 246.502.291 | 20.088 |

La mediana de la tabla es el promedio de los dos valores centrales:
256.170.229,5 ns (256,17 ms) para snapshots y 15.849,5 ns (15,85 μs) para
asignaciones concurrentes.

Una primera repetición con el contador previo dio snapshot mediano de 243,82 ms
y asignación concurrente de 14.242 ns; se conserva como evidencia histórica.
La tabla anterior corresponde a la versión con accounting no decreciente,
antes de añadir reconciliación automática.

Los perfiles específicos del snapshot a ~800k VMAs y de la asignación
concurrente mientras el refresh está bloqueado no se repitieron después de
añadir reconciliación. La matriz final sí mide la reconciliación a alta
cardinalidad durante el probe posterior a liberar 100k stacks del lote de
500k, además del rechazo de 1M. La recuperación y la coalescencia también se
validan con pruebas.

La primera ejecución del benchmark tras añadir el contador no decreciente fue
rechazada preventivamente en ~1.047.551 VMAs después de liberar un lote. No
hubo fallo ni daño de memoria: el snapshot conservaba deliberadamente el pico
anterior y la prueba no refrescaba tras `drop(batch)`. Entonces se ajustó el
benchmark para tomar un snapshot fuera de la región cronometrada tras soltar
cada lote. Además, la implementación final reconcilia automáticamente un
rechazo por presupuesto con un snapshot fuera del mutex y un segundo check;
la repetición final pasó. El preflight conservador no se debilitó.

Stack fijo continúa como única estrategia de producción. El diseño de
Dynamic Growable Stack se añadió como alternativa documental experimental en
[ADR 002](../decisions/002-stack-strategy.md); no hay allocator dinámico,
crecimiento automático, decommit ni slabs implementados.

## Auditoría de aceptación F2 y límites

- Overflow ejecutando código real sobre el stack de rutina no se puede validar
  hasta F3, que aportará context switching. La guard page actual solo prueba
  un fault aislado.
- La política fatal de fallo `munmap` en `Drop` tiene una prueba en proceso
  hijo con dirección no alineada; se verifica `SIGABRT` y el diagnóstico sin
  tocar un mapping válido.
- El camino de limpieza tras fallo de `mprotect` tiene ahora una prueba de
  allocator que inyecta el error, confirma la liberación del mapping parcial y
  conserva un stack vivo sin cambios.
- Las pruebas de integración cubren direcciones estables, no solapamiento,
  ownership RAII, liberación y contadores.
- No existe pool/free-list ni reutilización administrada por el allocator;
  `free_stacks` permanece en cero. Tras `munmap`, Linux podría asignar la misma
  dirección a una reserva posterior, pero el runtime no promete ni gestiona ese
  comportamiento. No se adoptó una política de reuse.
- La escalabilidad concurrente ya tiene una primera caracterización: diez
  muestras de ciclos de asignación/liberación con 1, 2, 4 y 8 workers, mientras
  un thread solicita snapshots. El throughput mediano cae de 395.468 ops/s con
  un worker a 244.117 ops/s con ocho; no escala monótonamente. La medición es
  del allocator fijo actual, no del scheduler ni de goroutines ejecutables. Los
  datos detallados y la salida bruta están en el
  [informe concurrente](./2026-11-14-allocator-concurrent.md).
- Dynamic Growable Stack continúa sin implementar; es una alternativa
  experimental documental, no capacidad de F2. Pool y reutilización tampoco
  están implementados; permanecen descartados para Fixed Stack según ADR 002
  hasta una decisión y medición comparativa futuras.
- F2 fue reabierta para la prueba de fallo fatal de `munmap`; este trabajo
  actualiza evidencia y verifica rutas adicionales, pero no registra una nueva
  aprobación de cierre. No declarar F2 cerrada sin aprobación explícita y sin
  revisar las puertas obligatorias del roadmap. El crecimiento automático y el
  overflow real quedan asignados a F3.

## Validación ejecutada

La ejecución completa con `./scripts/bench-all.sh` terminó con código 0:
todos los targets de benchmark registrados, los perfiles manuales
`high_vma_preflight_costs` y `vma_preflight_component_costs`, y la salida bruta
quedaron registrados en
[`runs/20261114T125938Z-3069942.log`](./runs/20261114T125938Z-3069942.log).
La suite rutinaria `RUSTUP_TOOLCHAIN=1.85.0 ./scripts/check.sh` se volvió a
ejecutar tras estos cambios; su resultado actual está registrado en el informe
de fase. Los dos perfiles manuales terminaron satisfactoriamente. La escala de
1M no se forzó si el preflight la rechazó.

La repetición posterior con instrumentación independiente por worker volvió a
ejecutar el runner completo. Sus resultados de 100k stacks retenidos y
espera/duración del mutex están en el
[informe de alta cardinalidad](./2026-11-14-allocator-high-cardinality.md);
su log, [`runs/20261114T132501Z-3150726.log`](./runs/20261114T132501Z-3150726.log),
se conserva como referencia independiente y no sustituye los datos históricos
de esta auditoría.

La comparación del prototipo de syscalls fuera del mutex, con preflight
habilitado en ambas rutas, está documentada por separado en el
[informe serial vs. dos fases](./2026-11-14-two-phase-stack-allocator.md).
