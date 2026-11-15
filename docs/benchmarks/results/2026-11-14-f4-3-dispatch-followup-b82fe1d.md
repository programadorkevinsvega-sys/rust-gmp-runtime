# F4.3 — Seguimiento de dispatch y aislamiento de mediciones

## Entorno y reproducibilidad

- Base del código: `b82fe1d`; ejecución sobre working tree con los cambios de
  ADR 015 sin commit.
- Comandos: `RUSTUP_TOOLCHAIN=1.85.0 cargo bench --bench
  machine_dispatch_stabilized` y `RUSTUP_TOOLCHAIN=1.85.0 cargo bench --bench
  workload_cpu`.
- Toolchain: `rustc 1.85.0 (4d91de4e4 2025-02-17)`.
- Sistema: Linux x86_64, kernel `7.0.0-42-generic`.
- CPU: 12th Gen Intel Core i5-12450H, 12 CPUs lógicas.
- Governor observado: `powersave`.
- Configuración: 30 muestras, 128 G por batch, 64 G de calentamiento,
  workers 1/2/4 y orden rotativo; stacks de 32 KiB.
- Workloads revisados: no-op, 64, 2.048 y 65.536 pasos de xorshift64* por G.
  El contador de validación se ejecuta en batches independientes, fuera de las
  muestras.

La salida completa, incluidas las muestras brutas, está en
[los datos de benchmark](./2026-11-14-f4-3-dispatch-followup-b82fe1d-raw.txt).
Las muestras de CPU sin dispatch están en
[los datos del workload aislado](./2026-11-14-f4-3-workload-cpu-raw.txt).
Una segunda ejecución está en
[la repetición del workload aislado](./2026-11-14-f4-3-workload-cpu-repeat-raw.txt).
Este resultado corresponde al working tree indicado, no a una revisión
inmutable; no se creó un commit para archivarlo.

### Corrección del workload sintético

La comprobación de proporcionalidad encontró que la recurrencia lineal anterior
(`wrapping_mul`/`wrapping_add`) se simplificaba al optimizar el loop desenrollado:
el IR combinaba ocho pasos originales en una sola transformación afín. La
versión previa de las cargas CPU queda supersedida y no debe usarse para
atribuir coste a 64, 2.048 o 65.536 operaciones.

Se sustituyó por xorshift64* (xor-shifts y multiplicación dependiente del valor)
y se midió directamente sin dispatch. El IR optimizado conserva cada paso; el
assembly x86_64 repite la secuencia de shifts, XOR y multiplicación dentro del
loop, en vez de colapsar ocho actualizaciones en una. Cada G sigue invocando
una vez su workload; la versión directa usa semillas opacas distintas de cero
por llamada para impedir que el compilador reutilice el resultado entre
llamadas y evitar el estado absorbente cero.

| Pasos por llamada | Mediana A (ns/batch) | Mediana B (ns/batch) | Aprox. ns/paso A/B |
|---:|---:|---:|---:|
| No-op | 95 | 118 | Resolución insuficiente |
| 64 | 13,594 | 14,535 | 1,65 / 1,76 |
| 2.048 | 540,290 | 552,604 | 2,06 / 2,11 |
| 65.536 | 17,471,840 | 17,783,385 | 2,08 / 2,12 |

Cada batch contiene 128 llamadas. El coste aproximado por paso resta la mediana
no-op y divide por 128 y por el número de pasos. Las dos ejecuciones muestran
un crecimiento monotónico y resultados por paso dentro de aproximadamente 7 %
entre sí para las cargas CPU. No-op y 64 pasos están cerca del ruido de medición;
la evidencia de proporcionalidad es más clara para 2.048 y 65.536 pasos. Esto
solo valida coherencia del workload sintético, no su equivalencia con cargas
reales. `perf stat -e cycles,instructions` no pudo acceder a los contadores: el
host tiene `perf_event_paranoid=4` y deniega eventos de hardware.

## Dispatch completo

Nanosegundos por batch de 128 rutinas; `completion_drain` incluye ejecución,
context switch, notificación y joins. No es un desglose aditivo. Cada rutina
invoca su workload una vez; los pasos xorshift mostrados son por G.

| Workers | Carga por G | Mediana | MAD | p95 | Máximo observado | Rango |
|---:|---|---:|---:|---:|---:|---:|
| 1 | No-op | 626,637 | 59,589 | 849,683 | 872,254 | 346,603 |
| 1 | CPU ligera (64) | 626,519 | 52,464 | 860,640 | 1,468,050 | 963,435 |
| 1 | CPU moderada (2.048) | 1,174,168 | 60,593 | 1,520,403 | 2,708,737 | 1,694,339 |
| 1 | CPU pesada (65.536) | 19,307,155 | 357,433 | 22,345,769 | 23,070,866 | 5,049,069 |
| 2 | No-op | 545,526 | 44,771 | 1,026,700 | 1,058,497 | 583,982 |
| 2 | CPU ligera (64) | 515,816 | 46,313 | 1,151,460 | 1,221,657 | 771,861 |
| 2 | CPU moderada (2.048) | 764,258 | 77,457 | 1,434,461 | 1,578,576 | 909,892 |
| 2 | CPU pesada (65.536) | 9,822,134 | 153,599 | 16,096,099 | 18,309,538 | 8,991,320 |
| 4 | No-op | 659,149 | 66,946 | 1,260,336 | 1,632,003 | 1,073,470 |
| 4 | CPU ligera (64) | 683,525 | 86,036 | 1,037,018 | 1,119,295 | 566,951 |
| 4 | CPU moderada (2.048) | 783,012 | 136,666 | 1,190,122 | 1,204,795 | 662,058 |
| 4 | CPU pesada (65.536) | 6,622,079 | 1,204,203 | 7,940,888 | 10,685,379 | 5,774,515 |

En las medianas, 2 workers son 12,9 % más rápidos que 1 en no-op y 49,1 %
más rápidos en CPU pesada. Cuatro workers son 20,8 % más lentos que dos en
no-op, pero 32,6 % más rápidos en CPU pesada. Cuatro workers mejoran 65,7 %
frente a uno en CPU pesada, pero no en no-op. El resultado depende del tamaño
del trabajo; no fija una política universal ni prueba escalabilidad general.

### Diferencias emparejadas exploratorias

Se emparejaron muestras por índice de ronda y se hizo bootstrap percentil de la
mediana de las diferencias (50.000 remuestreos). Son intervalos descriptivos
para esta ejecución; el orden rotativo no elimina todos los efectos temporales
del host y no se tratan como una prueba universal de significación.
El remuestreo usa `random.Random(20261114)` con una semilla común para las
comparaciones y límites de orden 1.251 y 48.750 (índices base uno de 50.000
remuestreos ordenados).

| Comparación | Diferencia mediana emparejada (ns/batch) | IC bootstrap 95 % |
|---|---:|---:|
| 1 → 2 workers, no-op | −62.977 | [−92.617, −5.437] |
| 2 → 4 workers, no-op | +126.930 | [+89.590, +223.594] |
| 1 → 4 workers, no-op | +71.247 | [+23.280, +106.055] |
| 1 → 2 workers, CPU pesada | −9.246.379 | [−9.598.903, −8.808.760] |
| 2 → 4 workers, CPU pesada | −4.277.074 | [−4.774.860, −3.046.456] |
| 1 → 4 workers, CPU pesada | −13.011.334 | [−13.872.376, −12.344.342] |

El delta CPU pesada menos no-op por configuración fue +18.715.807 ns para
1 worker (IC [+18.533.492, +18.942.146]), +9.321.379 ns para 2 workers
(IC [+9.197.553, +9.404.264]) y +5.470.063 ns para 4 workers
(IC [+4.603.757, +6.523.590]). Las comparaciones 64 pasos menos no-op
incluyen cero en sus intervalos emparejados; 2.048 pasos aumentan el tiempo en
1 y 2 workers, mientras el intervalo de 4 workers incluye cero. El benchmark
no identifica por sí solo qué componente produce las diferencias. Los datos
solo caracterizan estos workloads, este host y hasta cuatro workers; no prueban
escalabilidad general ni determinan cuántos Machines conviene usar en otros
entornos. Las métricas aisladas tampoco permiten atribuir causalmente el coste
al canal, al cambio de contexto o al scheduler del sistema operativo.

### Verificación de ownership y rollback

La inspección de la implementación y las pruebas confirman que:

- `reserve_for_dispatch` marca la reserva bajo el lock de lifecycle; clones no
  pueden usar `discard` ni `try_start` mientras está activa.
- El worker consume esa reserva al reclamar `Runnable → Running`.
- En un canal cerrado, `DispatchTask::reject` es el único responsable del
  rollback; el manager devuelve el error sin volver a descartar la rutina.
- Destruir un comando pendiente abandona la reserva y resuelve su join handle.
  Los tests comprueban además que la rutina queda terminal y que el worker
  completa una rutina posterior tras el intento de manipular un clon.
- Tanto completion normal como error liberan la referencia interna del worker
  antes de notificar, de modo que `join` no adelanta esa liberación.

Estas pruebas verifican los casos controlados de reserva, rechazo y destrucción
de tarea; no convierten los futuros estados de yield/reanudación o migración en
capacidades ya implementadas.

## Microbenchmarks aislados

Nanosegundos por batch de 128 operaciones:

| Componente | Mediana | MAD | p95 | Máximo observado | Rango |
|---|---:|---:|---:|---:|---:|
| Publicar G | 4,015 | 193 | 6,030 | 6,049 | 2,336 |
| Cola local P, enqueue/dequeue | 7,729 | 193 | 10,792 | 17,432 | 10,450 |
| Claim/yield de permiso | 10,235 | 424 | 14,501 | 22,166 | 12,710 |
| Ciclo terminal | 10,036 | 380 | 14,132 | 15,372 | 6,049 |
| `run_once` one-shot | 142,243 | 9,327 | 242,254 | 269,781 | 143,228 |
| Selección round-robin | 787 | 30 | 820 | 821 | 91 |

`run_once` no es el coste puro de Assembly. Referencias aisladas por batch:

| Referencia | Mediana | MAD | p95 | Máximo observado | Rango |
|---|---:|---:|---:|---:|---:|
| Llamada directa indirecta | 170 | 6 | 188 | 285 | 129 |
| `catch_unwind` normal | 167 | 7 | 174 | 198 | 43 |

Las referencias contextualizan llamada y captura de panic normal; no se deben
restar mecánicamente de `run_once` para estimar Assembly. `catch_unwind` sigue
siendo parte de la frontera segura. La mediana de `run_once` equivale a unos
1.111,3 ns por G al dividir el batch entre 128; es un coste normalizado del
camino completo, no del Assembly aislado. Por sí solo no identifica qué parte
optimizar ni constituye una meta de rendimiento.

Canales unidireccionales aislados, cada uno incluye envío/recepción, drenaje y
join de 128 mensajes; el spawn del thread queda fuera del cronómetro:

| Etapa | Mediana | MAD | p95 | Máximo observado | Rango |
|---|---:|---:|---:|---:|---:|
| Command delivery: caller → worker | 18,250 | 297 | 170,548 | 171,719 | 154,199 |
| Completion delivery: worker → caller | 19,628 | 315 | 69,018 | 302,005 | 283,158 |

No son mediciones de los canales de `MachineManager`; dispatch real se mide por
separado. El benchmark validó 1.536 entradas en batches adicionales sin un
contador compartido en el camino medido.

## Creación completa de G

Se separaron 30 batches de 128 G por configuración y workers. Se rota el orden
de tres fases: creación de 128 G con stacks simultáneamente vivos; dispatch/join
de G precreadas; y ciclo completo `Routine::new + dispatch + join + drop`.
El drop de los stacks creados sin dispatch se midió aparte. Los snapshots
VMA/memoria se toman fuera del cronómetro. La creación aislada tiene medianas
de 227,214–240,528 ns por batch, mientras que el ciclo completo queda en
810,717–844,471 ns; son fases distintas y no deben interpretarse como una
comparación aditiva o como evidencia de escalamiento.

| Workers | Fase | Mediana | MAD | p95 | Máximo observado | Rango |
|---:|---|---:|---:|---:|---:|---:|
| 1 | Crear | 240,528 | 14,928 | 286,172 | 387,468 | 174,402 |
| 1 | Dispatch/join precreadas | 358,178 | 29,045 | 537,344 | 587,342 | 287,672 |
| 1 | Crear+dispatch+join+drop | 835,204 | 46,099 | 966,695 | 1,175,831 | 476,702 |
| 1 | Drop creación aislada | 154,267 | 8,619 | 186,823 | 212,035 | 75,031 |
| 2 | Crear | 227,214 | 9,675 | 251,386 | 270,103 | 66,490 |
| 2 | Dispatch/join precreadas | 287,777 | 25,917 | 336,448 | 397,130 | 231,033 |
| 2 | Crear+dispatch+join+drop | 844,471 | 35,511 | 977,003 | 1,025,951 | 308,980 |
| 2 | Drop creación aislada | 150,333 | 8,938 | 213,775 | 248,706 | 112,692 |
| 4 | Crear | 238,470 | 9,235 | 464,193 | 496,162 | 292,690 |
| 4 | Dispatch/join precreadas | 176,487 | 22,420 | 240,964 | 264,382 | 145,024 |
| 4 | Crear+dispatch+join+drop | 810,717 | 31,796 | 1,604,314 | 1,703,870 | 980,019 |
| 4 | Drop creación aislada | 154,471 | 9,629 | 268,049 | 302,313 | 167,751 |

Cada configuración asignó y liberó 11.520 stacks y terminó con cero stacks
activos/reservados. En la fase de creación se mantuvieron 128 stacks activos
simultáneamente, con 4.718.592 bytes reservados en ese punto. VMA observado
antes/después: 67/65, 71/67 y 79/71 para 1, 2 y 4 workers. Esto no mide el
máximo de G ejecutando simultáneamente ni equivale al benchmark de creación de
threads nativos de `routine_lifecycle`. La ruta de finalización ahora libera la
referencia interna del worker antes de notificar el resultado; el test de
`join` confirma que, cuando no hay clones externos, el stack ya fue liberado al
retornar.
