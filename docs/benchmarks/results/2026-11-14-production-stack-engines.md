# Integración de motores de stacks de producción

## Alcance

La integración selecciona Fixed o Go-inspired mediante `StackConfig` y usa la
misma fachada `StackManager`/lease `Stack` desde `Routine`. Fixed permanece
como default. Go-inspired en esta ruta asigna desde el pool global; no usa ni
simula caches ligadas a Processors reales.

## Medición inicial de lifecycle (mismo proceso)

Comando: `cargo +1.85.0 bench --bench routine_lifecycle`.

La ejecución inicial calentó cada modo y tomó diez lotes de 64 operaciones en
un mismo proceso. Mide
construcción de `Routine`, publicación y transiciones `try_start`/`complete`;
no ejecuta closures sobre el stack, no hace context switching y no mide el
scheduler. El control crea y espera 64 threads nativos de Rust con closures
vacías. Son referencias contextuales, no workloads semánticamente equivalentes.
Los snapshots de memoria y VMA se leen fuera de cada intervalo medido.

Entorno: Rust 1.85.0, Linux x86_64, kernel
`7.0.0-42-generic`, 12 CPUs lógicas, revisión base `b82fe1d` con cambios de
trabajo sin commit. Los datos brutos completos están en
[`20261114T2310-production-engines-lifecycle.log`](./runs/20261114T2310-production-engines-lifecycle.log).

| Modo | Mediana por lote (64) | Rango observado | Mediana por operación | VMA al medir | Virtual reservado |
|---|---:|---:|---:|---:|---:|
| Fixed | 269,444 µs | 169,134–372,641 µs | 4,210 µs | 32 | 0 al terminar cada stack |
| Go-inspired | 27,198 µs | 22,267–27,532 µs | 0,425 µs (425 ns) | 34 | 4.456.448 bytes retenidos en 1 span |
| 64 threads nativos | 1.521,329 µs | 995,222–1.750,051 µs | 23,771 µs/thread | 57 en la mediana | stacks nativos del SO |

En Go-inspired se contaron 704 leases/releases, 703 reutilizaciones de slot y
64 slots libres. El dato de 65.536 bytes accesibles corresponde a un único slot
habilitado y reutilizado 703 veces; los otros 63 slots libres todavía no se
habían activado. La reserva de 4.456.448 bytes es el span completo, no 64 stacks
habilitados. La RSS observada fue 2.555.904 bytes y no se atribuye solo a stacks.

La mejora observada de aproximadamente 9,9x frente a Fixed corresponde
únicamente a este ciclo secuencial caliente de creación/lifecycle y reutiliza
slots. No demuestra una mejora general, escalabilidad concurrente, coste de
ejecución stackful ni que Go-inspired deba ser el default. La retención del span
y sus VMAs sigue siendo un coste explícito; `trim_to_retained_spans(0)` lo
recuperó al final del benchmark.

Esta ejecución no fue una comparación por procesos aislados; los valores se
conservan como histórico y no son la medición principal para comparar ambos
motores.

## Reconciliación del layout Go-inspired

Para el stack solicitado de 64 KiB, Linux reportó páginas de 4 KiB. El usable
alineado sigue en 65.536 bytes (16 páginas); cada slot añade una guard page de
4.096 bytes. Así, `stride = 65.536 + 4.096 = 69.632 bytes`; un span de 64 slots
mide `69.632 × 64 = 4.456.448 bytes = 4,25 MiB`. Toda esa dirección virtual se
reserva; inicialmente sus páginas son inaccesibles. Habilitar un slot hace
accesibles sus 16 páginas usables, no convierte esos bytes en RSS/residencia
física.

Esto reconcilia el snapshot histórico: 64 slots libres cuentan disponibilidad,
no activación. `accessible_virtual_bytes = 65.536` indica un slot activado;
los 63 restantes estaban libres pero sin activar. En el A/B, los lotes alcanzan
256 rutinas; tras completar y liberar el lote, el snapshot conserva cuatro
spans con 256 slots libres y activados, 4.096 páginas usables accesibles
(16 MiB), y 17.825.792 bytes reservados (17 MiB, incluidos 256 guard pages).
El benchmark verifica
`span_count × span_bytes == reserved_virtual_bytes` y publica todos los campos
en cada snapshot `layout`.

## Repetición con procesos aislados

Comando: `cargo +1.85.0 bench --bench routine_lifecycle`.

El benchmark actualizado lanza un proceso hijo por muestra y espera a que
termine antes de iniciar el siguiente. Ejecuta diez muestras por engine y
alterna el orden entre Fixed/Go-inspired. Cada proceso crea únicamente un
manager, realiza un lote de calentamiento y mide un lote idéntico de 64 ciclos
`Routine::new`/publicación/completado con stack solicitado de 64 KiB. El tiempo
de arranque del proceso y el calentamiento quedan fuera del cronómetro. No hay
coexistencia de engines, pools o mappings de sus muestras; ambos motores usan
el mismo binario y configuración pública. El benchmark no fuerza afinidad CPU
ni aísla cargas ajenas del sistema operativo.

Entorno: Rust 1.85.0 (`4d91de4e4`), revisión base `b82fe1d` con cambios de
trabajo sin commit, Intel Core i5-12450H, Linux x86_64/kernel
`7.0.0-42-generic`, 12 CPUs lógicas y governor `powersave`. Las diez muestras
por motor dieron:

| Motor | Mediana por lote (64) | Rango observado | Mediana por rutina | Reservado al snapshot | VMAs |
|---|---:|---:|---:|---:|---:|
| Fixed | 253,106 µs | 134,085–351,099 µs | 3,955 µs | 0 bytes | 32 |
| Go-inspired | 26,922 µs | 21,491–28,610 µs | 0,421 µs (421 ns) | 4.456.448 bytes retenidos | 34 |

Go-inspired fue aproximadamente 9,4x más rápido en la mediana de este ciclo
caliente y secuencial, pero el rango de Fixed fue amplio; por ello no se debe
interpretar la razón como una ventaja universal ni atribuir toda diferencia
exclusivamente al allocator. En cada muestra se observaron 128 leases y
liberaciones; al snapshot no quedaban stacks activos. Go-inspired retuvo 64
slots reutilizables en un span, mientras Fixed no retuvo mappings de stack.
La RSS del proceso varió entre muestras y no se atribuye exclusivamente a los
stacks. La salida completa está en
[`20261114T2329-production-engines-isolated.log`](./runs/20261114T2329-production-engines-isolated.log).
El tiempo por rutina es la mediana de valores por muestra truncados antes de
agregarse; dividir la mediana del lote entre 64 puede diferir en fracciones de
nanosegundo por el orden de agregación y redondeo.

## Comparación completa: dispatch stackful y escala

Comando: `cargo +1.85.0 bench --bench stack_engine_scalability`.

La matriz ejecuta un proceso aislado por cada combinación de motor y cantidad
de workers: Fixed/Go-inspired × 1, 2, 4, 8, 12, 16. Los hijos se ejecutan uno
por vez; cada uno crea un único manager de stacks y un único `MachineManager`.
Dentro del proceso mide diez muestras por cada combinación de workload y lote:
no-op, CPU de 64 iteraciones y CPU de 2.048 iteraciones; 32, 256 y 1.024
rutinas; stacks solicitados de 64 KiB. La comprobación de ejecución separada
verifica una entrada por rutina en los tres workloads. El cronómetro end-to-end
incluye la creación de rutinas/stacks, publicación, planificación, ejecución
real sobre el stack de rutina y join. Se registran también, sin sumarlos como
causas independientes, la asignación de stacks y el tramo dispatch-to-join.
Los escenarios calientan el engine y rotan su orden. Inicio/parada de workers y
snapshots de memoria quedan fuera de esas muestras.

Entorno: Rust 1.85.0, revisión base `b82fe1d` con cambios locales sin commit,
Intel Core i5-12450H, Linux x86_64/kernel `7.0.0-42-generic`, 12 CPUs lógicas,
governor `powersave`. En lotes de 1.024, medianas end-to-end (ms):

| Workers | Fixed no-op | Go-inspired no-op | Fixed CPU2048 | Go-inspired CPU2048 |
|---:|---:|---:|---:|---:|
| 1 | 6,759 | 1,484 | 11,791 | 5,967 |
| 2 | 6,174 | 1,493 | 7,624 | 3,704 |
| 4 | 7,332 | 1,674 | 7,490 | 2,117 |
| 8 | 9,295 | 2,507 | 9,312 | 1,663 |
| 12 | 8,777 | 3,015 | 9,233 | 2,364 |
| 16 | 9,454 | 3,444 | 9,736 | 3,123 |

Con 1.024 tareas no-op, Fixed alcanzó su mejor mediana con dos workers y
empeoró al subir a 16; Go-inspired estuvo prácticamente igual con uno y dos
workers y perdió throughput al aumentar la concurrencia. En CPU2048,
Go-inspired mejoró hasta ocho workers y luego retrocedió; Fixed alcanzó su
mejor resultado con cuatro y empeoró al subir a 16. Por tanto, el resultado
depende tanto del engine como de la carga: no hay escalabilidad monótona ni un
número de workers universalmente óptimo. Las diferencias son evidencia de este
workload y host, no una atribución causal a un subsistema ni una garantía de
mejora para aplicaciones.

La política de memoria también difirió. Al terminar el proceso de 16 workers,
Go-inspired conservaba 1.024 slots libres, 71.303.168 bytes reservados,
67.108.864 bytes accesibles y 2.146 VMAs; Fixed reportó cero bytes retenidos
por el manager de stacks y 100 VMAs de proceso. Los snapshots incluyen el
proceso completo: por ejemplo, el espacio virtual total también contiene las
reservas de stacks nativos de workers y no se atribuye solo al engine. La
retención del pool Go-inspired es un coste medido que throughput por sí solo no
refleja. Una prueba posterior con 1.024 rutinas precreadas confirmó los
71.303.168 bytes reservados y observó entre 2.086 y 2.147 VMAs antes del trim.
`trim()` liberó 15 spans y redujo la reserva a 4.456.448 bytes, pero retener el
span predeterminado mantuvo 128 VMAs más que la base; el trim a cero recuperó
los bytes y dejó el conteo VMA igual a la base del proceso. El experimento
aislado y sus límites se documentan en
[`2026-11-15-stack-trim-workers.md`](./2026-11-15-stack-trim-workers.md).

La salida contiene las 1.080 muestras, medianas, MAD, máximo observado,
asignaciones, ciclos de vida y snapshots de cada proceso:
[`20261114T2341-stack-engines-stackful-isolated.log`](./runs/20261114T2341-stack-engines-stackful-isolated.log).
Con diez muestras el máximo es el rango p95 por convención de rango, no una
estimación robusta de latencia de cola. El aislamiento evita que los dos
motores compitan entre sí, pero no elimina carga externa ni cambios de
frecuencia/temperatura del host. El benchmark mide la ejecución one-shot stackful
que actualmente ofrece `MachineManager`; no existen aquí pruebas de yield,
reanudación, canales, timers o red.

## A/B de ejecución completa de Routine

Se añadió una comparación específica porque medir `Routine::new` o adquirir un
lease no demuestra que la closure se ejecute en el stack real. El benchmark
despacha cada `Routine` al `MachineManager`; el worker entra por
`run_once_with_permit` sobre el stack de esa rutina, y luego se hace `join`.
Cada proceso prueba un solo motor y un único manager, de forma secuencial y
aislada del proceso del otro motor. La matriz usa 1, 2, 4 y 8 workers, 256
rutinas, diez muestras y cargas no-op, CPU64 y CPU4096. Una comprobación
independiente valida exactly-once. El lote precreado mide ejecución end-to-end
sin creación dentro del intervalo; `cold_full` incluye creación y asignación
desde un `StackManager` recién creado, mientras `hot_full` reutiliza el pool
calentado.

La corrida de rendimiento no habilita instrumentación:
[`20261114T2355-routine-execution-ab-uninstrumented.log`](./runs/20261114T2355-routine-execution-ab-uninstrumented.log).
Medianas end-to-end para el lote precreado de 256 rutinas (ms):

| Workers | No-op Fixed / Go-inspired | CPU64 Fixed / Go-inspired | CPU4096 Fixed / Go-inspired |
|---:|---:|---:|---:|
| 1 | 1,108 / 0,263 | 1,249 / 0,298 | 3,617 / 2,710 |
| 2 | 1,009 / 0,240 | 0,982 / 0,233 | 2,336 / 1,319 |
| 4 | 0,969 / 0,335 | 1,007 / 0,371 | 1,437 / 0,944 |
| 8 | 1,116 / 0,610 | 1,117 / 0,582 | 1,225 / 0,494 |

Para un worker y no-op, las medianas sin instrumentación de `cold_full`,
`hot_full` y `precreated` fueron 1,768 / 1,777 / 1,108 ms para Fixed y
1,235 / 0,295 / 0,263 ms para Go-inspired. El contraste frío/caliente muestra
el beneficio del pool Go-inspired en este workload; no debe extrapolarse a
otras cargas. El tiempo end-to-end incluye publicación, ejecución, join y la
liberación final de las referencias. No incluye el inicio del proceso ni la
creación del `StackManager`, informada aparte. Ningún motor escala
monótonamente en todas las cargas: estos resultados describen esta matriz y
este host, no una causa del coste ni una garantía de escalabilidad.

La segunda corrida habilita instrumentación opcional:
[`20261114T2356-routine-execution-ab-instrumented.log`](./runs/20261114T2356-routine-execution-ab-instrumented.log).
Desglose de medianas para 256 rutinas y un worker (µs):

| Fase / carga | Motor | Creación (pared) | Adquisición de stack (suma) | `run_once_with_permit` (suma) | Finalización (suma) | Dispatch (pared) | Join (pared) | End-to-end (pared) |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| cold_full / no-op | Fixed | 656,1 | 620,2 | 470,8 | 15,7 | 63,3 | 700,1 | 2.092,9 |
| cold_full / no-op | Go-inspired | 664,7 | 612,5 | 425,8 | 15,3 | 94,1 | 536,4 | 1.340,8 |
| precreated / no-op | Fixed | 522,4 | 494,1 | 553,7 | 17,6 | 55,0 | 760,7 | 1.329,3 |
| precreated / no-op | Go-inspired | 47,2 | 22,4 | 87,2 | 14,5 | 82,7 | 161,5 | 275,0 |
| precreated / CPU4096 | Fixed | 513,8 | 485,3 | 2.765,6 | 15,2 | 37,3 | 3.075,4 | 3.663,8 |
| precreated / CPU4096 | Go-inspired | 46,8 | 22,4 | 2.367,4 | 15,3 | 38,1 | 2.531,0 | 2.598,2 |

La instrumentación se guarda por rutina y solo existe al compilar con la
feature `runtime-benchmark-instrumentation`; estos números son diagnósticos,
no una comparación de rendimiento. La adquisición y finalización son
subintervalos de creación y `run_once_with_permit`, respectivamente; no se
suman de nuevo al end-to-end. Los tiempos `*_sum` agregan las rutinas y no son
tiempo de pared; pueden solaparse con el trabajo de otros workers. Los valores
de `creation_wall`, `dispatch_wall`, `join_wall` y `end_to_end_wall` sí son
tiempos de pared de cada lote. El benchmark valida exactamente una entrada por
rutina, pero esta comparación no cubre yield/reanudación, canales, timers o red.

La matriz aislada de adquisición/devolución del pool global frente a cachés
explícitas por worker y su diagnóstico de contención están en
[`2026-11-15-pool-contention.md`](./2026-11-15-pool-contention.md). Ese
experimento identifica coste del pool global en el microbenchmark, pero no
atribuye causalmente la degradación end-to-end del scheduler o `Routine`.
La comparación stackful de `Routine` con cachés reales asociadas a Processors
está en [`2026-11-15-routine-processor-cache.md`](./2026-11-15-routine-processor-cache.md);
mide el ciclo directo sobre workers Processor-bound, sin colas/dispatch de
`MachineManager`, por lo que tampoco sustituye el A/B del scheduler completo.

## Validación funcional

Las pruebas dirigidas comprueban selección explícita, default Fixed, métricas,
rechazo de preflight, retorno del lease, trim y creación de una rutina
Go-inspired. Una prueba unitaria adicional despacha y ejecuta la rutina en un
Machine worker sobre su stack Go-inspired y verifica una única ejecución,
finalización y liberación.
