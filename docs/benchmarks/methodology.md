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
Linux x86_64. Reserva y publica las rutinas antes del cronómetro; el coste
medido por `run_once` incluye trampoline, `catch_unwind`, lifecycle y primer
toque del stack. Divide ese coste por dos para mostrar una estimación por
transferencia, no un switch aislado. Registra cada muestra y resume p50, p95,
p99 (máximo observado con diez muestras), mínimo, máximo y dispersión. El valor
por defecto son 256 rutinas por muestra; se admite de 1 a 4096 para controlar
el uso de VMA. Registrar commit, `rustc -V`, hardware, kernel y governor junto a
la salida; no comparar el valor directo con `run_once` como workloads
equivalentes ni tomar esta primera línea base como resultado de scheduler.

`cargo bench --bench machine_dispatch` calienta cada configuración con 16
rutinas y luego mide diez batches de 128 rutinas ya reservadas y despachadas
mediante 1, 2 y 4 workers. El intervalo medido incluye
enqueue, transferencia de contexto one-shot, entrega del resultado y join;
excluye inicio/parada de workers y reserva de stacks. Cada entrada incrementa
un contador verificado. Es una línea base acotada del ciclo de dispatch, no una
comparación con threads nativos ni una medida de un scheduler reanudable.
Conservar la salida bruta y reportar el número de workers, coste por tarea,
throughput, mediana, máximo observado y dispersión con los metadatos del
entorno.

El test manual ignorado `high_vma_preflight_costs` separa lectura/parsing de
procfs, snapshot completo, checks del presupuesto, locks y syscalls de
`mmap`/`mprotect`/`munmap`; también compara lotes del allocator con preflight
habilitado/deshabilitado y presión alta de VMA. Se ejecuta explícitamente con
`cargo +1.85.0 test --release --lib high_vma_preflight_costs -- --ignored
--nocapture`. Puede reservar hasta 500k stacks simultáneos y no forma parte de
`scripts/check.sh`; revisar el presupuesto VMA local antes de correrlo.

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
