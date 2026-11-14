# PHASE REPORT

**Estado actual:** F2 cerrada por aprobación explícita del usuario el
2026-11-14, después de validar la prueba aislada del fallo de `munmap` durante
`Drop`, el accounting concurrente, la repetición de escala y los experimentos
de transferencia de cachés. No se inició implementación de F3.
**Antecedente:** F2 se reabrió para cubrir el fallo fatal de `Drop`; esa
cobertura y las verificaciones adicionales quedaron completadas antes de esta
aprobación.

## 1. Phase

F2 — Rutinas y gestión de stacks.

## 2. Objectives

Implementar el modelo inicial de rutinas y la propiedad de stacks estables en
Linux x86_64, sin scheduler ni context switching.

## 3. Architecture

- `Routine` comparte identidad, metadatos, ciclo de vida y owner del stack por
  `Arc`. Las transiciones se serializan con `Mutex`.
- `ExecutionPermit` representa una reclamación exclusiva de `Running`; completar,
  ceder, esperar o iniciar un syscall gestionado consume el permiso. Descartarlo
  activo hace terminal a la rutina como abandonada.
- `StackManager` reserva individualmente `guard + usable pages` con `mmap`, deja
  la guardia en `PROT_NONE` y habilita las páginas restantes con `mprotect`.
  `Stack` libera el mapeo mediante RAII.
- El allocator Fixed Stack de producción no tiene crecimiento, pool ni
  reutilización. Hay un pool/cache Go-inspired experimental, no promovido a
  producción. F2 tampoco implementa ejecución de closures, cambio de contexto,
  M, P ni scheduler.

## 4. Files created

- `src/routine/error.rs`, `src/routine/handle.rs`, `src/routine/permit.rs`.
- `src/routine/context_storage.rs`, `src/routine/scheduling.rs`,
  `src/routine/handle_tests.rs`.
- `docs/decisions/007-routine-context-and-scheduling.md`.
- `benches/scale.rs`, el experimento `benches/mapping_layout.rs` y sus módulos.
- `benches/allocator_concurrent.rs` y `scripts/bench-all.sh`.
- `docs/benchmarks/results/2026-11-14-allocator-concurrent.md` y el log bruto
  `docs/benchmarks/results/runs/20261114T125938Z-3069942.log`.
- Benchmarks y datos brutos en
  [`docs/benchmarks/results/2026-11-14-d9617743aba79bbdf62b3ea63c2a053cce5ef01c.md`](../benchmarks/results/2026-11-14-d9617743aba79bbdf62b3ea63c2a053cce5ef01c.md).
- `src/stack/error.rs`, `src/stack/mapping.rs`, `src/stack/metrics.rs`,
  `src/stack/process_metrics.rs`, `src/stack/protection/dispatch.rs`,
  `src/stack/size.rs`.
- `agente_rules.d/80-limite-archivos.md`.
- Este informe.

## 5. Files modified

- `Cargo.toml`, `.gitignore`, `AGENTS.md`, `README.md`, `PHASES.md`,
  `scripts/check-forbidden.sh`, `agente_rules.d/80-limite-archivos.md`,
  `docs/gmp-model.md`, `docs/context-switching.md`, `docs/stack-management.md`,
  `docs/decisions/002-stack-strategy.md`, `docs/benchmarks/methodology.md`.
- `src/lib.rs`, `src/routine/{id,lifecycle,metadata,mod,state}.rs`,
  `src/stack/{allocator,mod,protection}.rs`, `tests/routine_tests.rs`,
  `tests/routine_scheduling_tests.rs`, `tests/stack_tests.rs`,
  `benches/memory.rs`, `benches/routine_lifecycle.rs`.
- `src/routine/routine.rs` se renombró a `src/routine/handle.rs`;
  `src/stack/stack.rs` se renombró a `src/stack/mapping.rs`.
- `docs/stack-management.md`.

## 6. Features implemented

- IDs no nulos, estados/transiciones validados, metadatos básicos y resultados
  terminales `Completed`/`Abandoned`.
- Exclusión de ejecución con permiso único; referencias/permiso mantienen vivo
  el stack y `Dead` no puede volver a ejecutarse.
- Contexto estable asociado al stack: guarda el límite superior y
  reserva el campo de stack pointer guardado, que permanece vacío hasta el
  backend real de F3. No constituye un contexto ejecutable.
- Snapshots coherentes de planificación: entradas runnable, ejecuciones,
  yields, duraciones acumuladas, timestamp de finalización y razón/hora de
  bloqueo; wake y retorno de syscall limpian el estado de bloqueo.
- Stack page-aligned con guardia inferior, reserva individual estable y
  liberación RAII. Soporte implementado solo en Linux x86_64.
- Contadores de asignación/liberación/stack activo y mediciones de memoria
  virtual, RSS aproximada y VMA en Linux.
- Límite automatizado de 200 líneas físicas para `.rs` y `.sh`.

## 7. Tests executed

- `./scripts/check.sh` con Rust 1.85.0: formato, escáner de patrones
  prohibidos, Clippy, tests de todos los targets y documentación.
- `cargo +1.85.0 bench --bench memory`.
- `cargo +1.85.0 bench --bench routine_lifecycle`.
- `RUST_GMP_RUN_SCALE=1 cargo bench --bench scale` (1k, 10k, 100k y 500k stacks;
  diez muestras por nivel).
- `RUST_GMP_RUN_SCALE=1 cargo bench --bench mapping_layout` (mapeo individual
  frente a reserva agrupada; diez muestras por disposición y nivel).
- `./scripts/bench-all.sh` (todos los targets registrados y los dos benchmarks
  manuales VMA; conserva la salida bruta en `docs/benchmarks/results/runs/`).
- `cargo check --bench mapping_layout` y revisión manual de los invariantes FFI
  del prototipo.
- `git diff --check`.

## 8. Test results

`./scripts/check.sh` terminó con código 0. Pasaron 18 pruebas unitarias, 6 de
rutina, 3 de planificación/contexto y 3 de integración de stacks (30 pruebas).
La nueva prueba fuerza un error de `mmap` con un tamaño imposible, verifica que
se incremente el contador de fallos y que no queden reservas activas; pruebas
unitarias verifican el retorno de error del syscall `munmap`, incluyendo una
dirección no alineada rechazada por Linux. El caso de guardia escribe en proceso
hijo y verifica `SIGSEGV`.
`cargo +1.85.0 check --all-targets` también pasó. Los benchmarks tienen diez
muestras por grupo; el máximo observado se identifica como máximo y no como
estimación robusta de p99. Los resultados brutos, medianas corregidas,
comparación experimental de topología y límites están en el informe enlazado.

## 9. Memory considerations

Cada stack reserva `usable_size + una página`; los bytes habilitados son espacio
virtual con permisos, no memoria residente. `/proc/self/statm` ofrece RSS
aproximada. La nueva matriz mide hasta 500k reservas simultáneas de 8 KiB útiles
sin tocar páginas; no mide 1M, recursión, locales grandes ni stacks usados por
una rutina en ejecución.

## 10. Performance considerations

Se midió preparación de 64 reservas y ciclo de vida de 64 rutinas frente a
threads nativos. Son operaciones distintas: las rutinas no ejecutan código y
no hay context switch. Los números no representan throughput ni rendimiento
del runtime. Las medianas por lote de stacks individuales fueron 1.63 ms,
18.98 ms, 197.81 ms y 922.99 ms para 1k, 10k, 100k y 500k, respectivamente.
Un experimento aislado encontró menor tiempo de reserva agrupada, pero VMA
equivalentes; no basta para justificar un cambio de allocator.

## 11. Safety review

- `unsafe` queda encapsulado en la frontera Linux x86_64 de stack/memoria; las
  pruebas verifican alineación, límites y protección en un proceso hijo.
- Una reserva `mmap` fallida se propaga y contabiliza; el error de `munmap` se
  verifica con un rango no alineado que el kernel rechaza sin liberar mappings.
- El error real de `mprotect` sobre un rango no alineado se propaga; la prueba
  libera la reserva de prueba después del fallo.
- Pruebas existentes verifican bounds estables, no solapamiento, ownership
  hasta la última referencia y liberación/accounting.
- El experimento de agrupación no cambia el allocator de producción; el FFI de
  benchmark tiene owner único RAII, valida rangos y aborta si falla `munmap`.
- Tamaños, conversiones y contadores usan validación/cálculo comprobado; los
  errores de lock y parsing conservan contexto.
- Si `munmap` o la contabilidad fallan durante `Drop`, el proceso aborta porque
  el trait no permite propagar un error de liberación.
- Esta funcionalidad no es un sandbox y no ejecuta código de usuario en el
  stack reservado.

## 12. Known limitations

- El slot de F2 solo guarda el límite superior; el stack pointer
  guardado y registros CPU están vacíos hasta F3. No hay contexto ejecutable.
- Los tiempos acumulados miden intervalos entre transiciones protegidas por el
  lock, no uso de CPU; no existe scheduler que despache o despierte tareas.
- Recursión, variables locales grandes y formato/`println!` sobre el stack
  requieren context switching y quedan pendientes para una fase posterior.
- La política fatal de fallo `munmap` en `Drop` se prueba en proceso hijo
  forzando un rango rechazado por Linux, sin liberar ni alterar un mapping
  válido. Fixed Stack no tiene crecimiento ni reutilización; Linux podría
  elegir una dirección liberada para un `mmap` posterior, pero no es una
  garantía ni una política del allocator.
- La escala medida llega a 500k; 1M no se midió porque se estima que excedería
  el límite local de VMA. El benchmark agrupado mantiene VMA por slot debido a
  `mprotect` individual, con un VMA adicional para el mapping base.
- Fixed Stack no tiene pool ni reutilización por decisión del ADR 002. El
  pool/cache Go-inspired se mantiene como experimento y no altera producción.
  El overflow real de una rutina tampoco se prueba antes de F3; F2 sí comprueba
  que la guard page faulta.
- `STATUS.md` no está presente en esta rama. El estado actual queda reflejado en
  `README.md`, `PHASES.md` y este informe.

## 13. Pending work

- La ejecución sobre el stack, recursión, locals grandes y formato/`println!`
  sobre G requieren el context switching de F3 y no bloquean F2.
- La comparación de mapeos individuales y agrupados está medida como
  experimento de syscalls/VMA, no como allocator completo. No se adopta slab:
  no reduce VMA con protección por slot y no se han probado sus garantías de
  ciclo de vida/retención como implementación de producción.
- 1M de reservas no se midió porque se estima que excedería el límite VMA de
  este entorno; la matriz reproducible llega a 500k. Repetir esa medición solo
  en un entorno con recursos suficientes y sin cambiar límites globales.
- Crear el commit y PR de cierre siguiendo el flujo Git del repositorio. No se
  creó commit ni PR en esta actualización.

## 14. Next phase

F3 — Context switching. El cierre de F2 no inicia ni aprueba por sí mismo
trabajo de implementación de F3.

## 15. Addendum: diagnóstico y preflight de VMA

Este addendum registra una ampliación posterior solicitada para F2. La
aprobación de cierre indicada en la sección 16 incluye esta ampliación. No se
cambió la topología de stacks ni se inició F3.

### Auditoría y decisiones

- Antes del cambio, `StackManager` ya reservaba por stack con `mmap(PROT_NONE)`,
  habilitaba la región usable mediante `mprotect`, liberaba con `munmap` y
  observaba memoria de proceso/VMA solo como métricas opcionales. No consultaba
  `/proc/sys/vm/max_map_count` ni tenía admisión preventiva.
- Se mantuvo la decisión de F1/ADR 002: mapeos individuales, stack fijo y sin
  pool/slab. El allocator de producción no usa el prototipo agrupado.
- La lectura Linux usa `std::fs`; VMA vacío, malformado, ilegible, límite
  inválido y overflow se informan como errores. No se convierte la falta de
  diagnóstico en capacidad ilimitada.
- El preflight se habilita por defecto, retiene 1.024 VMA configurables y puede
  desactivarse explícitamente. Al desactivarlo, no se afirma protección
  preventiva. Con preflight habilitado, un fallo de procfs rechaza la reserva.
- El coste máximo estimado de dos VMA por stack se deriva del `mmap` individual
  y del `mprotect` que separa guardia y región usable. No equivale al número
  real: el kernel puede fusionar o dividir mappings. El presupuesto usa el
  máximo estimado del runtime y el snapshot observado, con margen.
- Un mutex compartido por los clones del manager serializa admisión,
  `mmap`/`mprotect`, accounting y `munmap`; el estado interno evita que reservas
  del propio runtime gasten capacidad duplicada. La lectura de procfs ocurre
  fuera de ese mutex y el snapshot se refresca en diagnósticos explícitos y
  cada 262.144 operaciones de lifecycle. Los mappings externos pueden cambiar
  entre observaciones.
- El preflight es orientativo, no reserva capacidad del kernel. Una llamada
  nativa todavía puede fallar. Los errores reales se propagan y una reserva
  parcial se limpia antes de devolver error; un fallo de `munmap` en `Drop`
  conserva la política fatal previa del runtime.

### Archivos añadidos y modificados

- Añadidos `src/stack/{config,admission,diagnostics,vma}.rs`,
  `src/stack/{allocator_tests,mapping_tests,performance_tests,performance_scale_tests,vma_tests}.rs`,
  `src/stack/error/display.rs`, `src/stack/admission_refresh.rs`,
  `src/stack/diagnostics_released_capacity_tests.rs` y `tests/vma_tests.rs`.
- Modificados `src/stack/{allocator,error,mapping,metrics,mod,process_metrics}.rs`,
  `src/stack/diagnostics_tests.rs` y `benches/scale.rs`.
- Actualizados `docs/stack-management.md`, `docs/memory-model.md`,
  `docs/decisions/002-stack-strategy.md`, `docs/benchmarks/methodology.md`,
  `README.md`, `PHASES.md`, este informe y
  `docs/benchmarks/results/2026-11-14-vma-performance.md`.
- No se añadieron dependencias directas ni transitivas, FFI, syscalls crudas,
  threads crudos ni cambios de sysctl.

### Métricas y semántica

`vma_limit`, `vma_current`, `vma_remaining` y `vma_utilization_percent` vienen
del snapshot procfs. `runtime_mapping_count` cuenta mappings activos que posee
el manager; `runtime_vma_estimate` es una cota por mapping, no un conteo exacto
atribuible al runtime. `free_stacks` permanece en cero porque no existe pool.
`allocation_failures` (alias compatible de `failed_allocations`) cuenta
fallos de `allocate`, incluidos rechazos preventivos; `vma_preflight_rejections`
cuenta además las consultas preventivas explícitas rechazadas. Los bytes
reservados y RSS siguen siendo métricas distintas.

### FFI y `unsafe`

Esta ampliación no añadió `unsafe` ni FFI. El backend preexistente Linux x86_64
mantiene aislados `sysconf`, `mmap`, `mprotect` y `munmap` en
`src/stack/protection.rs`, con sus bloques `unsafe` documentados; el diagnóstico
VMA solo lee procfs mediante `std`. La prueba de guard page preexistente escribe
intencionalmente en un proceso hijo, no se modificó.

### Pruebas y estado de criterios

- Unitarias: límite válido/vacío/malformado/overflow/zero; líneas válidas,
  archivo vacío y contenido malformado; remaining saturado y porcentaje;
  margen y presupuesto interno; 8 threads compitiendo por un presupuesto
  sintético de 10 VMAs, donde solo se admiten cinco reservas de coste dos.
- Integración Linux: límite y recuento reales cotejados con procfs, métricas
  antes/después, reserva y liberación real, 8 threads con ciclos intercalados,
  rechazos concurrentes cerca de capacidad cero y preservación de un stack
  activo. `tests/stack_tests.rs` conserva la prueba de fallo real de `mmap`;
  pruebas previas verifican errores de `mprotect` y `munmap`.
- La ruta fatal de `munmap` en `Stack::drop` se prueba en un proceso hijo: se
  entrega al owner una dirección deliberadamente no alineada (`NonNull::dangling`),
  que Linux rechaza con `EINVAL`. La prueba verifica `SIGABRT` y el diagnóstico
  fatal de `munmap`; no afecta mappings válidos ni depende de una carrera o de
  fallos ambientales.
- La limpieza parcial después de un fallo de protección tiene una prueba del
  allocator: inyecta el error de `mprotect`, verifica que se libera el mapping
  parcial, actualiza fallos y conserva las direcciones/accounting de otro stack
  todavía vivo.
- La auditoría midió que procfs no se consulta por stack: `allocate` hace
  admission check por operación, pero usa el snapshot cacheado y suma el coste
  monótono de las reservas posteriores al inicio de su lectura. Las liberaciones
  no descuentan ese coste hasta un snapshot posterior. Se refresca en
  diagnósticos explícitos o cada 262.144 operaciones.
  Antes de la optimización, el snapshot de ~800k VMAs mantenía el mutex hasta
  ~191 ms y un waiter se bloqueaba ~193 ms. Ahora procfs se lee fuera del
  mutex: la repetición final de alta cardinalidad dio snapshots de
  244,40–292,87 ms (mediana corregida 256,17 ms) y una asignación concurrente
  mediana de 15.850 ns (rango 13.265–23.054 ns). Una repetición preliminar
  anterior al ajuste de accounting había dado 14.242 ns; se conserva como dato
  histórico.
  Las pruebas verifican que la asignación termina antes de liberar al lector,
  que cuatro solicitantes comparten un refresh y que un error de lectura
  despierta waiters y permite recuperación con el siguiente refresh. El thread
  que inicia el refresh aún espera su lectura.
- En la repetición final, lotes no pareados de 100k midieron 203,89–277,79 ms
  con preflight (mediana 228,46 ms) y 198,20–262,80 ms sin preflight (mediana
  213,55 ms). La baja latencia de admisión concurrente no elimina el coste de
  throughput para el solicitante del refresh. Se añadieron snapshots fuera de
  cronómetro tras cada lote liberado para que la política conservadora no use
  el pico temporal anterior como si fuera el estado actual.
- Un rechazo preventivo por un snapshot inflado ahora fuerza o comparte un
  refresco y repite la admisión una sola vez. Las pruebas verifican recuperar
  capacidad después de liberaciones y que cuatro solicitantes concurrentes de
  reconciliación producen una sola lectura. Si el segundo cálculo también
  rechaza, el runtime devuelve el error preventivo.
- Dos pruebas nuevas fuerzan que una liberación ocurra durante el refresh
  iniciado tras un rechazo, y que ocho threads asignen concurrentemente
  mientras otro snapshot está en curso, liberando cuatro mappings antes de
  publicarlo. El estimate sintético conserva el coste de todas las reservas
  sin cancelarlo con `munmap`; luego una reconciliación real observa las
  liberaciones y permite asignar de nuevo. Esto comprueba que las carreras de
  asignación/liberación no subestiman el coste ni causan rechazo permanente por
  accounting obsoleto.
- La cadencia anterior de 65.536 operaciones hizo lotes de 100k con 400k stacks
  base ~642,62 ms con preflight frente a ~215,28 ms deshabilitado. Con la
  cadencia 262.144, las medianas fueron ~402,37 ms y ~206,78 ms. El coste sigue
  siendo visible; el snapshot es una observación advisory, nunca garantía.
- La repetición de escala con cadencia 262.144 registró medianas de reserva de
  2.158.457, 19.872.199, 188.546.820 y 1.095.233.846 ns para 1k/10k/100k/500k;
  medianas de liberación de 1.272.284, 12.233.795, 113.214.687 y 579.119.777 ns.
  En 500k hubo 1.000.031 VMA observadas, 500.000 mappings del manager y
  1.000.000 VMA estimadas. Los datos y las comparaciones independientes se
  conservan en [`auditoría de rendimiento VMA`](../benchmarks/results/2026-11-14-vma-performance.md).
- Se volvió a ejecutar `RUST_GMP_RUN_SCALE=1 cargo +1.85.0 bench --bench
  scale` tras integrar la reconciliación. Diez muestras por nivel dieron
  medianas de reserva/liberación de 1.926.819/1.261.023 ns (1k),
  20.212.120/14.375.977 ns (10k), 199.757.884/136.702.428 ns (100k) y
  1.145.301.788/700.261.944 ns (500k). No hubo fallos ni rechazos en estas
  escalas. Tras liberar parcialmente 100k stacks del lote de 500k, el probe
  de 25k stacks reconcilió el snapshot obsoleto y pasó el segundo check en
  mediana de 205.639.675 ns (máximo observado 242.339.143 ns). El preflight
  de 1M rechazó tras reconciliar en 16.698 ns, sin intentar reservar. Datos
  brutos y VMA por muestra están en el informe de rendimiento VMA. A 500k se
  registraron 6.144.000.000 bytes virtuales reservados,
  4.096.000.000 bytes accesibles y RSS aproximada activa de
  22.458.368–26.447.872 bytes; la RSS de `statm` no es memoria atribuible
  exactamente a los stacks. El benchmark es serial: no mide throughput concurrente.
- 1M se rechazó preventivamente: 2.000.000 VMA estimadas exceden el límite
  real observado de 1.048.576, con margen de seguridad de 1.024. No se hizo la
  reserva ni se provocó un fallo nativo. En las escalas medidas no hubo
  rechazos ni fallos nativos.
- El informe VMA inicial conserva sus datos brutos y el baseline F2 permanece
  independiente. La diferencia original de +125,37% a 500k no se reprodujo:
  la repetición anterior dio +18,7% respecto del baseline original. La
  repetición final dio +24,1% respecto del baseline y no es pareada; no atribuye
  el cambio a un único factor.
- El límite consultado no garantiza reservas futuras. No se modificaron límites
  globales. La prueba de overflow ejecutando código sobre el stack de una
  rutina permanece pendiente de F3: aún no hay context switching ni ejecución
  de closures sobre esos stacks. Los benchmarks completos de ciclo de vida de
  slabs tampoco se hicieron; no se adoptó esa estrategia.
- Dynamic Growable Stack quedó descrito como alternativa experimental en ADR
  002 y en gestión de stacks. No se implementó allocator dinámico, crecimiento
  automático, decommit ni slabs; Fixed Stack sigue siendo la única estrategia
  de producción.
- El benchmark concurrente nuevo mide ciclos reales de `allocate`/`Drop` con
  1/2/4/8 workers y refresh de VMA simultáneo. En diez muestras de 5.000
  operaciones por worker, el throughput mediano fue 395.468, 261.202, 242.180
  y 244.117 operaciones/s, respectivamente. El throughput no escala
  monótonamente con workers y el experimento no justifica rediseñar el
  allocator. Las pruebas de accounting concurrente siguen siendo evidencia
  separada de corrección, no de rendimiento. Ver [el informe concurrente](../benchmarks/results/2026-11-14-allocator-concurrent.md)
  y su [log bruto](../benchmarks/results/runs/20261114T125938Z-3069942.log).
  Dynamic Growable Stack sigue siendo documental/experimental y no está
  implementado. El allocator Fixed Stack de producción no reutiliza mappings;
  el pool/cache Go-inspired existe únicamente como implementación experimental.
- La decisión de sacar procfs del mutex y usar accounting conservador durante
  refresh quedó registrada en [ADR 008](../decisions/008-vma-refresh-and-admission.md);
  se incluye en el cierre de F2 aprobado en la sección 16.
- Revalidación final tras las pruebas de accounting concurrente y el benchmark
  paralelo: `RUSTUP_TOOLCHAIN=1.85.0 ./scripts/check.sh` salió con código 0.
  Los benchmarks manuales costosos quedaron ignorados por diseño en la suite
  rutinaria; Clippy, formato, escáner de patrones/límites, tests y rustdoc
  terminaron satisfactoriamente. Todos los archivos `.rs` cumplen el límite de
  200 líneas.
- Las comprobaciones adicionales están validadas y forman parte del cierre de
  F2 registrado en la sección 16. No inicia F3 ni altera sus criterios.

## 16. Cierre aprobado y decisión del target de refill

El usuario aprobó explícitamente el cierre de F2 el 2026-11-14 después de
completar la revisión solicitada. No se inicia F3 en este cambio.

El torneo de transferencia comparó targets fijos, adaptación experimental,
refills sincronizados y vidas intercaladas. Para el presupuesto de caché de
32 KiB, el target predeterminado continúa siendo **1 slot**. El target de
**2 slots** queda disponible únicamente como configuración experimental del
cache Go-inspired; sus resultados no justifican cambiar defaults ni el
allocator Fixed Stack de producción. No se cambia el código de producción.
Los resultados y límites están en
[el informe de transferencia](../benchmarks/results/2026-11-14-refill-transfer.md),
y el registro de la decisión experimental está en
[ADR 014](../decisions/014-refill-batch-sizing.md).

La verificación final de esta ampliación incluyó el torneo completo,
`cargo fmt --check`, Clippy con `-D warnings` y
`cargo test --lib --features allocator-benchmark -- --test-threads=1`
(76 pasaron; 2 ignoradas). No se promueve Go-inspired a producción ni se
declaran implementados context switching, ejecución sobre el stack o scheduler.
