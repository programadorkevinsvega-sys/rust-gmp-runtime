# Metodología de benchmarks

## Registro reproducible

Por benchmark conservar commit, `rustc -V`, hardware/CPU, kernel o SO, governor,
configuración, comando exacto, código de workload, calentamiento, al menos 10
repeticiones, datos brutos y variación. Reportar mediana, p99 y dispersión. No
inventar resultados ni comparar workloads con semánticas diferentes sin
explicarlas. Guardar informes en
`docs/benchmarks/results/<fecha>-<commit>.md`.

## Cargas y línea base

Preparar workloads para CPU pequeña, espera/suspensión, canales, carga mixta,
miles de tareas, red y carga sostenida. El benchmark usa `harness = false` y
`std::time::Instant`. Calentar antes de medir, ejecutar al menos 10 veces y
mantener el trabajo y la concurrencia comparables. Cuando aporte contexto,
comparar con threads nativos de Rust usando el mismo número de workers.

`cargo +1.85.0 bench --bench routine_lifecycle` mide diez muestras independientes
por engine. El proceso coordinador inicia un hijo por muestra y espera su salida
antes de iniciar el siguiente; el orden Fixed/Go-inspired se alterna para limitar
sesgo sistemático por orden. Cada hijo crea solo un `StackManager`, calienta un
lote y cronometra un lote de 64 ciclos `Routine::new`/publicación/completado,
con stack solicitado de 64 KiB. No se ejecutan ambos engines concurrentemente
ni comparten pools, mappings o recursos del proceso; arranque y calentamiento
quedan fuera del intervalo medido. El ciclo y la configuración son iguales,
aunque la estrategia interna de asignación y retención propia de cada engine
forma parte del resultado. Los snapshots VMA/RSS se toman fuera del cronómetro.
El Go-inspired mide su pool global desde el caller, no caches explícitas ni
caches asociadas a Processors reales. Reporta cada muestra, mediana, máximo
observado y métricas de memoria. El benchmark no mide ejecución stackful ni
escalabilidad del scheduler; el aislamiento por proceso tampoco controla carga
externa del sistema ni variación de frecuencia/temperatura de CPU. Los resultados
y datos brutos de la comparación aislada están en
[`2026-11-14-production-stack-engines.md`](./results/2026-11-14-production-stack-engines.md).

`cargo +1.85.0 bench --bench stack_engine_scalability` complementa ese
microbenchmark con ejecución real en stacks de rutina. Lanza secuencialmente
un proceso por cada combinación de engine (Fixed/Go-inspired) y workers
(1, 2, 4, 8, 12, 16); nunca mantiene dos engines o grupos de workers vivos a la
vez. Cada proceso crea un único `StackManager` y `MachineManager`, con stacks
solicitados de 64 KiB. Mide diez lotes para cada combinación de workload
(no-op, CPU de 64 iteraciones, CPU de 2.048 iteraciones) y tamaño (32, 256,
1.024 rutinas). Las rutinas se crean antes de publicar la ráfaga; el intervalo
end-to-end incluye creación/asignación, dispatch, ejecución stackful y join.
Se reportan aparte la asignación de stacks y el tiempo desde dispatch hasta
join. La configuración se calienta, el orden de escenarios rota por muestra y
se ejecuta una comprobación independiente que verifica exactamente una entrada
por rutina. Inicio/parada de workers y snapshots de memoria se informan aparte,
no forman parte del tiempo end-to-end.

Con diez muestras, el valor etiquetado como p95 corresponde al máximo observado
por la convención de rango usada; no es una estimación robusta de cola. Este
benchmark cubre dispatch one-shot sobre el stack de cada Routine, que es el
contrato actual de `MachineManager`; no prueba yield/reanudación de rutinas,
canales, timers ni red. Aislar procesos elimina competencia entre motores en la
prueba, pero no controla carga externa, frecuencia ni temperatura del host.
El informe y la salida completa se guardan en
[`2026-11-14-production-stack-engines.md`](./results/2026-11-14-production-stack-engines.md)
y [`20261114T2341-stack-engines-stackful-isolated.log`](./results/runs/20261114T2341-stack-engines-stackful-isolated.log).

`cargo +1.85.0 bench --bench routine_execution_ab` compara el ciclo de ejecución
stackful de `Routine` en procesos secuenciales separados por motor y cantidad de
workers (1, 2, 4, 8). Cada proceso prueba 256 rutinas y diez muestras por carga
(no-op, CPU64, CPU4096), con stacks solicitados de 64 KiB. La closure se ejecuta
en el stack real mediante `run_once_with_permit`; una comprobación separada
verifica exactly-once. `cold_full` mide creación/asignación y ejecución con un
`StackManager` recién creado por muestra; `hot_full` reutiliza el pool calentado;
`precreated` excluye la creación de rutinas de su end-to-end. El intervalo
end-to-end incluye dispatch, ejecución, join y liberación de referencias; el
inicio del proceso y la construcción del `StackManager` se informan aparte.

Para desglose por etapas, ejecutar una corrida distinta con
`cargo +1.85.0 bench --features runtime-benchmark-instrumentation --bench routine_execution_ab`.
No mezclarla con las cifras de rendimiento: la feature registra tiempos por
rutina y añade perturbación. La adquisición de stack está incluida en creación;
la finalización del permiso está incluida en `run_once_with_permit`. Sus sumas
por rutina no son tiempo de pared y pueden solaparse entre workers. No se
cronometra cada closure ni se añaden contadores globales al camino de producción.
Los resultados y logs de ambas corridas están en
[`2026-11-14-production-stack-engines.md`](./results/2026-11-14-production-stack-engines.md),
[`20261114T2355-routine-execution-ab-uninstrumented.log`](./results/runs/20261114T2355-routine-execution-ab-uninstrumented.log)
y [`20261114T2356-routine-execution-ab-instrumented.log`](./results/runs/20261114T2356-routine-execution-ab-instrumented.log).

`cargo +1.85.0 bench --bench stack_trim_workers` mide retención y recuperación
con el backend Go-inspired, stacks de 64 KiB, spans de 64 slots y 1.024
rutinas precreadas antes del dispatch. Ejecuta las closures en sus stacks reales
con un `MachineManager` aislado de 1, 2, 4, 8, 12 o 16 workers; cada proceso
valida exactly-once, mide memoria/VMA, aplica `trim()` y después
`trim_to_retained_spans(0)`. Tres procesos por configuración comprueban
reproducibilidad básica, no significancia estadística. El benchmark separa
reservado/accesible de RSS: los snapshots de proceso completo incluyen memoria
ajena al allocator. El informe y las muestras están en
[`2026-11-15-stack-trim-workers.md`](./results/2026-11-15-stack-trim-workers.md)
y [`20261115-stack-trim-workers.log`](./results/runs/20261115-stack-trim-workers.log).

`cargo +1.85.0 bench --bench pool_contention` compara en procesos aislados y
orden alternado el acquire/return del pool Go-inspired global frente a cachés
explícitas por worker; no ejecuta el scheduler ni representa caches asociadas a
Processors reales. Usa seis cantidades de workers, diez muestras por
configuración y 1.024 operaciones por worker, con stacks de 64 KiB. La corrida
con `--features allocator-benchmark` mide aparte esperas/posesión del mutex y
etapas; sus temporizadores y contadores perturban el rendimiento, y las sumas
por worker no son tiempo de pared. No mezclar sus cifras de throughput con las
de la corrida sin instrumentación ni atribuirlas directamente al scheduler.
Los resultados y límites están en
[`2026-11-15-pool-contention.md`](./results/2026-11-15-pool-contention.md)
y los logs emparejados
[`sin instrumentación`](./results/runs/20261115-pool-contention-paired-uninstrumented.log)
y [`con instrumentación`](./results/runs/20261115-pool-contention-paired-instrumented.log).

`cargo +1.85.0 bench --bench routine_processor_cache_ab` compara el ciclo
stackful `Routine::new`/`publish`/`run_once` en workers enlazados a Processors
reales, con pool global y con un `ProcessorStackCache` único por worker. La
rutina se crea y ejecuta en el worker propietario; el lease se devuelve a esa
misma caché al completar. Los hijos alternan los modos, usan 1/2/4/8/12/16
workers, 256 rutinas y diez muestras por workload (no-op, CPU64, CPU4096).
Tres matrices completas dan 30 muestras por configuración. No instrumenta el
intervalo cronometrado. Este benchmark evita dispatch de
`MachineManager`, sus colas, stealing y el join handle por rutina; es una
comparación controlada allocator+Routine, no una prueba end-to-end del
scheduler. Sus resultados y logs están en
[`2026-11-15-routine-processor-cache.md`](./results/2026-11-15-routine-processor-cache.md)
y los logs de la [primera](./results/runs/20261115-routine-processor-cache-ab-uninstrumented.log),
[segunda](./results/runs/20261115-routine-processor-cache-ab-repeat2.log) y
[tercera](./results/runs/20261115-routine-processor-cache-ab-repeat3.log) matriz.

`cargo +1.85.0 bench --bench machine_processor_cache_ab` compara el dispatch
real con stack manager global y con cachés Go-inspired por Processor. Los dos
modos usan `MachineManager`, `dispatch_new`, stacks de 64 KiB, los mismos
workers y workloads no-op/CPU64/CPU4096; cada lote incluye creación sobre el
worker seleccionado, publicación, ejecución stackful, finalización y join.
Cada modo/cantidad de workers corre en procesos aislados, tres matrices con 30
muestras por workload (90 por celda), y orden de modo alternado entre matrices.
No se añaden timers por rutina ni instrumentación diagnóstica a la ruta medida;
se conserva el accounting normal del allocator en ambos modos. Los contadores
de exactamente-una-vez y balance de leases se comprueban en trabajo separado
del intervalo cronometrado. El informe y los logs están en
[`2026-11-15-machine-processor-cache.md`](./results/2026-11-15-machine-processor-cache.md)
y los logs de la [matriz 1](./results/runs/20261115-machine-processor-cache-ab.log),
[matriz 2](./results/runs/20261115-machine-processor-cache-ab-repeat2.log) y
[matriz 3](./results/runs/20261115-machine-processor-cache-ab-repeat3.log).
El benchmark no aísla el scheduler de la asignación; mide el camino completo,
y no atribuye causalidad a un componente individual.

Las pruebas diferenciales deben usar un modelo de referencia determinista que
represente los contratos documentados de cierre de canales, selección,
despertar, cancelación y bloqueo. Documentar las divergencias respecto al
modelo; no afirmar equivalencia de comportamiento a partir de una sola cifra.

## Métricas

Spawn, ejecución, latencia/overhead de planificación, cambios de contexto,
throughput, CPU, espera/contención, RAM por tarea/total, espacio virtual
reservado, páginas comprometidas/tocadas, RSS, stacks/slabs/VMA, fragmentación,
fallos y tiempos de creación/planificación/finalización. Reportar por separado.

## Política

Primero medir y perfilar; optimizar solo cuellos de botella reproducibles y
medir de nuevo con el mismo método. Los parámetros 8 KiB iniciales y 256 KiB
virtuales de stack son candidatos de experimento, no defaults ni garantías.
Pruebas progresivas de escala objetivo: 1k, 10k, 100k, 500k y 1M G, solo cuando
los recursos existentes lo permitan. Registrar vm.max_map_count observado, sin
modificar límites globales.

`cargo bench --bench scale` omite las muestras de escala de forma predeterminada
para que la verificación rutinaria no reserve grandes cantidades de memoria
virtual. Ejecutarlo con `RUST_GMP_RUN_SCALE=1` mide 1k, 10k, 100k y 500k stacks
y considera 1M solo después de una admisión preventiva del Stack Manager. Cada
muestra separa reserva y liberación, y registra VMA/RSS antes, durante y
después; si el preflight rechaza una escala, se registra el rechazo y no se
fuerza la reserva. El preflight usa el límite y snapshot observados, pero no
garantiza capacidad frente a mapeos concurrentes del proceso.

`RUST_GMP_CONTEXT_BENCH_ITERATIONS=256 cargo bench --bench context_switching`
mide diez muestras de llamadas directas y ejecuciones one-shot `run_once` en
Linux x86_64, tanto con stacks fríos como con cada página usable pre-tocada
antes del cronómetro. Reserva y publica las rutinas antes de medir; ambos costes
incluyen trampoline, `catch_unwind` y lifecycle, mientras solo el caso frío
incluye el primer toque de páginas. Divide cada coste por dos para mostrar una
estimación por transferencia, no un switch aislado. Registra cada muestra y
resume p50, p95, p99 (máximo observado con diez muestras), mínimo, máximo y
dispersión. El valor por defecto son 256 rutinas por muestra; se admite de 1 a
4096 para controlar el uso de VMA. Registrar commit, `rustc -V`, hardware,
kernel y governor junto a la salida; no comparar el valor directo con
`run_once` como workloads equivalentes ni tomar esta línea base como resultado
de scheduler.

El test manual `interleaved_context_switch_backend_comparison` mide el backend
anterior y el actual en 20 muestras intercaladas, con dos stacks de 32 KiB
pre-tocados y 1.000.000 roundtrips por backend y muestra. El tiempo se toma sobre
el loop de ida/vuelta y se divide entre dos para estimar ns por transferencia;
reporta tres decimales, mediana muestral y diferencia emparejada con signo. No
incluye `run_once`, `catch_unwind` ni page faults. Es una comparación local de la
secuencia de instrucciones, no sustituye mediciones de aplicación ni demuestra
una mejora si la diferencia emparejada queda dentro de la variabilidad
observada. Ejecutar con
`cargo +1.85.0 test --release --lib interleaved_context_switch_backend_comparison
-- --ignored --nocapture`.

`cargo bench --bench machine_dispatch` calienta cada configuración con 16
rutinas y luego mide diez batches de 128 rutinas ya reservadas y despachadas
mediante 1, 2 y 4 workers. El intervalo medido incluye
enqueue, transferencia de contexto one-shot, entrega del resultado y join;
excluye inicio/parada de workers y reserva de stacks. Cada entrada incrementa
un contador verificado. Es una línea base acotada del ciclo de dispatch, no una
comparación con threads nativos ni una medida de un scheduler reanudable.
Conservar la salida bruta y reportar el número de workers, coste por tarea,
throughput, mediana, máximo observado y dispersión con los metadatos del
entorno. Esta línea base histórica no se modifica con el benchmark de
estabilización.

`RUSTUP_TOOLCHAIN=1.85.0 cargo bench --bench machine_dispatch_stabilized`
mide 30 muestras de batches de 128 G, tras 64 G de warm-up por combinación.
Rota el orden de 1, 2 y 4 workers y rota no-op, CPU ligera (64 iteraciones),
CPU moderada (2.048 iteraciones) y CPU pesada (65.536 iteraciones). El contador
de validación corre en batches adicionales, fuera de las muestras cronometradas,
para no introducir contención artificial en las entradas medidas. Reporta
tiempo de submit (publicación,
selección round-robin y envío al worker), drenaje de finalizaciones
(ejecución, notificación y joins) y end-to-end. Por separado reporta
microbenchmarks de publicación, round-robin atómico, cola P, ciclo de permiso,
ciclo terminal y `run_once`. También separa dos transferencias `mpsc`:
`command_delivery_send_and_drain` envía desde el caller a un thread receptor;
`completion_delivery_send_and_drain` envía desde el thread emisor al caller.
El arranque del thread queda fuera del cronómetro; se incluye envío/recepción,
drenaje y join. Cada etapa es un modelo unidireccional aislado, no mide las
colas reales de `MachineManager`; dispatch real permanece medido por separado.
Agrega referencias directas de llamada y
`catch_unwind` normal para contextualizar `run_once`, que sigue incluyendo el
trampoline, la transferencia de contexto y el lifecycle. Estas referencias no
son medidas puras del ensamblador y no justifican retirar `catch_unwind`.
Los microbenchmarks son diagnósticos aislados, no sumandos del end-to-end.
Para cada métrica informa mediana, MAD, p95, máximo observado y rango
máximo-mínimo; no estima p99 con 30
muestras. Conserva la salida cruda en
`docs/benchmarks/results/<fecha>-f4-3-stabilization-raw.txt` y registra
toolchain, commit, CPU, kernel y governor. El benchmark verifica el total de
entradas en batches adicionales para detectar trabajo perdido o duplicado sin
añadir un contador compartido a las mediciones de ejecución.
Los workloads de CPU usan una recurrencia xorshift64* dependiente del valor
para evitar que la optimización colapse varios pasos en una sola transformación.
`RUSTUP_TOOLCHAIN=1.85.0 cargo bench --bench workload_cpu` mide las mismas
operaciones directamente, sin dispatch, con semillas opacas no nulas y
variables entre llamadas para evitar reutilización de resultados y el estado
absorbente cero. Inspecciona IR y assembly
optimizados al cambiar el workload; el conteo del loop por sí solo no prueba que
cada iteración mantenga el trabajo previsto.
También registra 30 batches de 128 G por cantidad de workers para cuatro
alcances: crear stacks/rutinas y medir su drop, dispatch/join de rutinas
precreadas, `Routine::new + dispatch + join`, y el drop medido por separado.
El batch de creación conserva las 128 G simultáneamente y registra su pico de
stacks activos/bytes reservados. Los snapshots, VMA y checks de asignaciones y
liberaciones se toman fuera de los intervalos. No equivale al benchmark de
creación de threads nativos de `routine_lifecycle`; tampoco mide el pico de G
simultáneamente en ejecución porque cada dispatch es one-shot y el batch crea
las tareas antes de unirlas.

El test manual ignorado `high_vma_preflight_costs` separa lectura/parsing de
procfs, snapshot completo, checks del presupuesto, locks y syscalls de
`mmap`/`mprotect`/`munmap`; también compara lotes del allocator con preflight
habilitado/deshabilitado y presión alta de VMA. Se ejecuta explícitamente con
`cargo +1.85.0 test --release --lib high_vma_preflight_costs -- --ignored
--nocapture`. Puede reservar hasta 500k stacks simultáneos y no forma parte de
`scripts/check.sh`; revisar el presupuesto VMA local antes de correrlo.
En `vma_preflight_component_costs`, cada modo de allocator es una sola muestra
de 10.000 ciclos secuenciales `allocate`/`drop`, cronometrada de extremo a
extremo. El manager habilitado obtiene un snapshot inicial dentro de la primera
asignación y reutiliza el snapshot para las restantes; no son 10.000 lecturas de
procfs ni diez repeticiones independientes. La cadencia de refresh cuenta
operaciones de lifecycle, con una asignación y una liberación por ciclo, y es
262.144 operaciones; las consultas explícitas de snapshot/metrics fuerzan una
observación nueva.

El test manual `high_vma_parser_comparison` compara la validación byte-oriented
actual con la implementación previa basada en `split_whitespace` y
`from_str_radix`, sobre 800.000 entradas sintéticas y sin reservar mappings.
Ejecutarlo con `cargo +1.85.0 test --release --lib high_vma_parser_comparison
-- --ignored --nocapture`. Ambos parsers deben contar todas las líneas; los
tests unitarios de VMA conservan los casos malformados.

`RUST_GMP_RUN_SCALE=1 cargo bench --bench mapping_layout` ejecuta la comparación
experimental individual/agrupada con la misma matriz. Solo mide la secuencia de
reserva/protección y VMA, no sustituye un benchmark del allocator con ownership,
liberación, retención y fallos parciales.

`scripts/bench-all.sh` ejecuta todos los targets de benchmark registrados en
Cargo, incluidos los workloads de escala y layout, y después ambos benchmarks
manuales ignorados de VMA. Antes de empezar comprueba el margen para el caso de
alto VMA; si no alcanza 1.010.000 mappings teóricos disponibles, aborta sin
ejecutar pruebas. Cada ejecución conserva la salida bruta en
`docs/benchmarks/results/runs/`. Requiere Linux, procfs y Rust 1.85.0.

`allocator_concurrent` mide diez muestras de ciclos concurrentes de reserva y
liberación con 1, 2, 4 y 8 workers. Un thread adicional fuerza snapshots VMA
repetidos durante cada muestra. Registra throughput, latencias por operación,
coste y cantidad de refreshes, y VMA antes/después. Es una medición del
allocator actual bajo contención y no un benchmark de goroutines del scheduler.
Reporta p50, p95 y p99 sobre las latencias individuales de cada muestra; el
máximo observado se informa por separado.

`allocator_high_cardinality` compara el allocator serial de referencia y el
prototipo experimental de reserva en dos fases, conservando 100k stacks vivos
con preflight VMA habilitado en ambos. Mide ciclos de asignación/liberación con
1, 2, 4 y 8 workers, diez muestras de 5k operaciones por worker, con un
snapshotter concurrente adicional apagado y encendido. El prototipo mantiene
contabilidad pendiente durante `mmap`/`mprotect` y `munmap`; solo confirma
asignaciones o liberaciones después del syscall exitoso. Rechaza la configuración
base si el coste VMA estimado y el margen exceden el límite observado; no fuerza
reservas. La feature opt-in `allocator-benchmark` habilita contadores
independientes por worker para esperas/adquisiciones del mutex, duración de sus
tramos críticos y tiempos/cantidades de `mmap`, `mprotect` y `munmap`. El
snapshotter usa contadores separados. Cada muestra incluye mappings/bytes y VMA
antes/después, además de deltas de fallos de asignación, fallos de cleanup y
rechazos preventivos. La instrumentación añade overhead; los resultados son
diagnósticos, no rendimiento de producción sin instrumentación. Los tiempos se
reportan por worker/syscall, normalizados por llamadas; p50/p95/p99 y máximo se
calculan por operación.

`scripts/bench-go-inspired-stack.sh` ejecuta el torneo de cuatro motores:
Fixed serial, Fixed dos fases, Block/Arena y Go-inspired. El target
`allocator_tournament` registra escalas de 1k/10k/100k/500k, reutilización
tocando todas las páginas y concurrencia con 100k stacks retenidos, 1/2/4/8
workers y diez muestras. En muestras con snapshots, los workers ejecutan ciclos
continuos de asignación/liberación mientras un thread independiente toma
exactamente cinco snapshots; no hay barreras que los detengan entre lecturas.
Cada worker completa una operación antes de sincronizar el comienzo del
snapshotter, y sigue trabajando hasta que terminan las cinco lecturas. La
preparación de la cache por worker termina antes de iniciar el cronómetro. El
throughput usa las operaciones realmente completadas y el tiempo total de la
muestra. La ejecución instrumentada registra la latencia de todas las
operaciones hasta la cuota configurada y de una de cada ocho operaciones
posteriores, para mantener suficientes observaciones de cola sin almacenar
cada ciclo de snapshots largos. Reporta throughput, p50/p95/p99/máximo, estado
de VMA/bytes y errores disponibles por motor. Las métricas instrumentadas de
espera/posesión de locks y de `mmap`/`mprotect`/`munmap` se recogen en
ejecuciones separadas del throughput no instrumentado. En Go-inspired cada
worker posee una cache explícita; esto simula la propiedad de P, no un
Processor del scheduler. El torneo incluye muestras de caché caliente y de
refill forzado para presupuestos de 32, 64, 128 y 256 KiB (configurables con
`RUST_GMP_TOURNAMENT_CACHE_KIB`): en estas últimas cada ciclo devuelve los slots locales al pool
global antes del siguiente `allocate`, por lo que throughput y latencia
incluyen tanto el flush como la ruta de refill y son un límite de estrés, no
una medida de amortización en el uso normal. También mide ráfagas simultáneas
de 2× y 4× el target efectivo de refill por worker: el worker mantiene vivos
los stacks de la ráfaga y reutiliza su caché entre ráfagas, sin vaciarla entre
operaciones. El cronómetro se detiene antes de vaciar las cachés al final de
cada muestra; el tiempo de flush y sus contadores instrumentados se reportan
por separado. Reporta la distribución de latencias de ráfaga y operaciones por
solicitud de refill. Registra hits, misses, solicitudes y batches de refill,
slots transferidos, solapamiento, contención del lock del pool y fallos de
asignación/limpieza; los deltas se comparan con el estado previo y posterior
de la muestra. El número de ráfagas por worker es 256 por defecto y se ajusta
con `RUST_GMP_TOURNAMENT_BURSTS`. Los escenarios no instrumentados
reportan throughput/latencia de la ruta; sus contadores de tiempos de lock están
desactivados y se recogen por separado en la variante instrumentada. La fase de
transferencia incluye tres experimentos adicionales. El refill adaptativo
compara el target fijo predeterminado, el target máximo permitido por la
capacidad local y un controlador experimental por ProcessorStackCache; usa
demanda `[1, 4, 16, 16, 4, 1, 0, 0]`, dobla el target después de dos refill
agotados consecutivos, lo reduce a la mitad después de dos ventanas de baja
demanda/ociosidad y limita el target a la capacidad de caché. Los epochs se
ajustan con `RUST_GMP_TOURNAMENT_DEMAND_EPOCHS` (256 por defecto). Este
controlador solo pertenece al benchmark: no cambia el default ni ejecuta una
política automática del runtime.
Las olas sincronizadas vacían las cachés entre rondas y sincronizan a todos los
workers antes de cada refill para medir transferencias contendidas; incluyen
asignación, retorno al cache y flush al pool. Se ejecutan 128 olas por defecto,
ajustables con `RUST_GMP_TOURNAMENT_REFILL_WAVES`, y se verifica el número
exacto de solicitudes y slots transferidos.
El experimento de vida intercalada asigna un stack por operación, usa el
calendario de vida `[1, 4, 16]`, libera handles vencidos mientras hay otros
vivos y compara flush de caché cada 1, 8 y 64 operaciones. Reporta throughput,
latencia por operación, cache hits/misses, batches, locks, errores y accounting.
Estos intervalos representan retención en operaciones y no tiempos de pared.

`cargo +1.85.0 bench --bench scheduler` caracteriza F5 tras 32 tareas de
calentamiento por combinación. Por defecto ejecuta 30 muestras de 256 G con
1, 2, 4, 8 y 12 workers, en orden rotativo, para workloads no-op y CPU de 64
pasos. Se puede ajustar la matriz con
`RUST_GMP_SCHEDULER_SAMPLES` y `RUST_GMP_SCHEDULER_WORKERS` (lista de cantidades
separadas por comas). Los managers de todas las configuraciones se inician
antes de medir; los no usados permanecen aparcados. Las rutinas y stacks se
crean antes del intervalo; se reportan submit, drain de joins, end-to-end y
throughput. Solo el modo `latency_instrumented` mide p50/p95 por tarea desde el
inicio de `dispatch` hasta el comienzo del closure; esa latencia incluye una
`Instant` y una escritura atómica por entrada.
Los batches `throughput_uninstrumented` no crean timestamps, atomics ni un
contador por rutina dentro del closure; verifican publicación, selección y
finalización mediante deltas de SchedulerStats y joins. Los batches
`latency_instrumented` miden distribución por rutina. Además, `parked_wake`
espera hasta que todos los workers del manager estén aparcados, publica una G y
mide dispatch-a-entrada antes del join; esta cifra incluye reserva y publicación,
notificación, despertar/planificación del hilo, selección y entrada, no el
coste aislado de despertar ni de selección.
También valida entradas exactly-once y deltas de publicaciones, selecciones,
finalizaciones y tareas por P; registra robos, parking, notificaciones y
ocupación pico acumulada de las colas. Los picos son acumulativos desde el
arranque de cada manager, no picos independientes por muestra. Es una
caracterización local; aunque la matriz amplia aumenta las muestras y workers,
no constituye una comparación formal de políticas ni evidencia de escalabilidad
universal. La salida completa se guarda en `docs/benchmarks/results/runs/`; los
resultados y su interpretación van en un informe separado, por ejemplo
[la caracterización F5 con modos separados](./results/2026-11-14-f5-scheduler-separated-modes.md).

`cargo +1.85.0 bench --bench scheduler_isolated` ejecuta un solo manager por
proceso y limita los lotes a 32, 256, 1.024 y 4.096 tareas. Cada configuración
se lanza en procesos independientes cambiando
`RUST_GMP_SCHEDULER_WORKERS`; `RUST_GMP_SCHEDULER_SAMPLES` controla las muestras
dentro de cada proceso. Ráfaga y flujo se ejecutan por separado con no-op y
CPU64. En ráfaga se publica el lote mientras las entradas esperan en una
compuerta, y se libera después de completar la publicación; flujo permite que
las tareas empiecen mientras continúa la publicación. No son cargas de
aplicación ni miden la selección aislada. El benchmark
`cargo +1.85.0 bench --bench scheduler_steal --features scheduler-benchmark`
fuerza desequilibrio precargando una cola local y haciendo que otros Ps roben;
mide selección/transferencia más barreras, sin ejecutar closures. El API normal
publica en la injection queue y no permite fijar un P. Los contadores de
intentos fallidos y exitosos de robo y tareas seleccionadas por P se reportan
por batch. `RUST_GMP_SCHEDULER_THIEVES` barre el número de thieves,
`RUST_GMP_SCHEDULER_STEAL_BATCHES` configura el límite de transferencia del
benchmark y `RUST_GMP_SCHEDULER_TASK_COUNTS` y
`RUST_GMP_SCHEDULER_SAMPLES` seleccionan tamaño y repeticiones. Para comparar
tiempos, ejecutar cada par thieves/batch en procesos frescos.
`RUST_GMP_SCHEDULER_TRACE=true` registra el propietario inicial, cada
transferencia y el P que selecciona cada rutina. También registra espera en las
barreras del harness; no es una medición de idle del scheduler. El trazado añade
trabajo bajo el lock y sus tiempos son solo diagnósticos. El follow-up y sus
límites están en
[la caracterización de stealing](./results/2026-11-14-f5-stealing-characterization.md);
la matriz previa de procesos aislados está en
[la matriz F5 aislada](./results/2026-11-14-f5-scheduler-isolated-matrix.md).
`cargo +1.85.0 bench --bench scheduler_attribution --features scheduler-benchmark`
permite comparar procesos frescos variando `RUST_GMP_ATTRIBUTION_WORKERS`,
`RUST_GMP_ATTRIBUTION_PUBLISHERS`, `RUST_GMP_ATTRIBUTION_STEALING`,
`RUST_GMP_ATTRIBUTION_TASKS`, `RUST_GMP_ATTRIBUTION_SAMPLES` y
`RUST_GMP_ATTRIBUTION_WORK_UNITS`. Mantiene fijo el total de tareas y
contrasta publicación serial/concurrente, stealing habilitado/deshabilitado y
trabajo útil por closure. Excluye la creación de rutinas y threads del intervalo,
sin instrumentar tiempos en el hot path. Reporta duración por publisher,
selecciones por P, transferencias y tiempos de submit/drain/end-to-end; valida
exactly-once y queues vacías. Cada combinación workers/publishers/stealing se
debe ejecutar en proceso nuevo. El control de stealing solo existe con la
feature de benchmark y no cambia la política normal; añade una rama de control
en la selección, por lo que las cifras absolutas no equivalen a producción.
No atribuye causalidad por sí solo: comparar tiempos con la corrida separada de
`scheduler-diagnostics` y reportar la variación entre procesos.
La feature `scheduler-diagnostics` y
`cargo +1.85.0 bench --bench scheduler_diagnostics --features scheduler-diagnostics`
ejecutan una matriz de diagnóstico separada sobre `MachineManager` real. Mide
espera/retención del lock de colas, selección, actividad y tiempo sin trabajo por
P, parking y robos. Preasigna las rutinas y sincroniza los publishers; cada
combinación workers/publishers debe ejecutarse en un proceso fresco. Las
publicaciones se atribuyen al P de origen round-robin. Los contadores son
atómicos, cache-padded y particionados por P; se actualizan después de liberar
el lock de colas, sin contador atómico global. Las llamadas a `Instant` siguen
añadiendo perturbación, así que estos resultados no son comparaciones de
rendimiento. No se informa latencia de unpark porque `Condvar` no identifica al
waiter notificado. Los resultados y limitaciones están en
[la revisión de Assembly y diagnóstico del scheduler](./results/2026-11-14-assembly-and-scheduler-diagnostics.md).
La comparación de causas entre publishers, stealing y granularidad de trabajo
está en [la atribución de causas F5](./results/2026-11-14-f5-cause-attribution.md).
Las fases de demanda y vidas permanecen separadas para distinguir la política
de tamaño del lote de la duración de vida de los stacks.
La fase de políticas de retención compara 0/1/4/16 spans, vacía las cachés de forma
coordinada, reduce explícitamente el target a un span como señal simulada de
presión y verifica VMA, espacio virtual reservado y RSS antes de liberar todo.
En la fase de escala Go-inspired
también vacía ocho cachés explícitas y libera spans vacíos después de soltar
los handles. Compara VMA, espacio virtual, RSS y contabilidad antes/después;
esta fase mide recuperación posterior y no reduce VMAs mientras los stacks
siguen vivos. El script aborta antes de ejecutar si el host no tiene
headroom para la escala de 500k, y persiste la salida completa en
`docs/benchmarks/results/runs/`. La matriz de transferencia de cachés ya se
ejecutó; sus mediciones y limitaciones están en
[el informe de transferencia](./results/2026-11-14-refill-transfer.md). El
torneo no se debe usar para concluir que un motor es más rápido en cargas de
goroutines reales: todavía no ejecuta el scheduler G-M-P.

Los archivos de ejemplo sin target Cargo registrado no son benchmarks
ejecutables y no se presentan como parte de la suite.

Los resultados originales de F2 se conservan como baseline independiente. El
benchmark de escala con preflight agrega tiempos separados de asignación y
liberación, snapshots de VMA/RSS antes/durante/después, y contadores de rechazos
y fallos. Diez repeticiones reportan mediana, máximo observado y dispersión;
el máximo no se presenta como p99 robusto.

`RUST_GMP_MEMORY_STACKS=8192 RUST_GMP_MEMORY_SAMPLES=10 cargo bench
--bench allocator_memory --features allocator-benchmark` mide diez muestras
por condición de RSS en procesos hijos independientes. Toca cada página usable
de 8.192 stacks y registra RSS desde una línea base por proceso: activos,
libres en pool global o cache local, después de flush, trim por targets 0/1/4/16
y liberación total. Los ocho caches del benchmark simulan ownership local; no
son Processors del scheduler. El RSS se lee de `/proc/self/statm`, es aproximado
y corresponde al proceso entero, no solo al allocator. El informe conserva las
muestras brutas y separa espacio virtual reservado de RSS. No usar estos datos
para elegir defaults de producción sin validar el ciclo de vida real de los
Processors y repetir en otros entornos.
