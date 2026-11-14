# Roadmap de implementación — Rust GMP Runtime

Este roadmap desglosa la especificación de [SPEC.md](./SPEC.md) en fases y subfases verificables. Es un plan de trabajo, no evidencia de que las características ya existan. El scaffold actual no debe presentarse como runtime funcional.

## Alcance y reglas que no cambian

- Runtime propio **M:N stackful**, organizado alrededor de G-M-P; no un thread pool presentado como runtime, ni un wrapper, ni tareas `async`.
- **Cero crates externos**, directos o transitivos, tanto en el runtime como en pruebas y benchmarks. Usar Rust y las APIs/ABI nativas verificadas; no depender de `libc` ni de bindings externos.
- Se permiten Rust, `core`, `alloc`, biblioteca estándar, inline/global Assembly, `asm!`, `global_asm!`, declaraciones `extern` y APIs nativas del SO. Aislar/documentar syscalls y FFI; herramientas externas de análisis pueden usarse manualmente, nunca como dependencia del runtime.
- Primera plataforma: **Linux x86_64**. Las demás plataformas son trabajo posterior, no compatibilidad implícita.
- Stacks con direcciones estables mientras estén vivos. Nunca mover ni copiar un stack activo sin una demostración formal de validez de todas sus referencias.
- Las rutinas migrables no conservan el TLS de una Machine. No prometer compatibilidad genérica con estado thread-local ni recursos con afinidad; especificar cualquier mecanismo de pinning/task-local y sus límites antes de implementarlo.
- Context switching propio; Assembly y FFI mínimos, aislados, auditados y documentados. La lógica del scheduler permanece en Rust.
- Preferir APIs de `std` donde basten; FFI/syscalls nativas se justifican operación por operación. No crear threads con syscalls crudas.
- `unsafe` mínimo y encapsulado, con invariantes, ownership, alineación y vida útil documentados. No sacrificar seguridad por rendimiento.
- Prevenir data races, use-after-free, double-free, punteros colgantes, corrupción de stacks, UB, deadlocks, liberación prematura, aliasing inválido y referencias expiradas.
- No afirmar que una interfaz, stub o prueba de creación de estructuras equivale a una característica implementada.
- Implementar progresivamente; no avanzar si fallan los criterios de salida de la fase anterior.
- No simular suspensión, preemption, I/O no bloqueante ni compatibilidad de plataformas. Declarar límites cuando no haya una solución validada.
- No confundir memoria virtual reservada, memoria comprometida y memoria residente; guard pages no significan stack growth recuperable.
- Netpoller debe separar readiness (`epoll`, `kqueue`) de completion (IOCP); tratar por separado sockets, pipes y archivos regulares.

## Secuencia y dependencias

```text
F1 Arquitectura
  -> F2 Rutinas y stacks
  -> F3 Context switching
  -> F4 Machines y processors
  -> F5 Scheduler
  -> F6 Suspensión y preemption
  -> F7 Sincronización, canales y select
  -> F8 Timers
  -> F9 Netpoller Linux
  -> F10 Gestión de memoria
  -> F11 Medición y optimización
  -> F12 Estabilización y aceptación Linux x86_64
  -> F13 Portabilidad progresiva (posterior y por plataforma)
  -> F14 Evaluación futura de scopes (optativa; depende de F12, no de F13)
```

F1 es una puerta obligatoria: no se implementa el scheduler completo ni se inicia F2 antes de que su documentación, decisiones, riesgos y plan de pruebas estén revisados. F13 no está dentro de la aceptación de la primera plataforma. F14 es una extensión optativa posterior a F12 y nunca se adelanta.

## Criterios transversales para cada subfase

Antes de implementar cada módulo, registrar responsabilidad, algoritmo, estructuras, interfaces, ownership, sincronización, consumo esperado, riesgos y pruebas. Después, compilar, ejecutar las pruebas pertinentes, revisar `unsafe`/FFI, documentar limitaciones y medir cuando corresponda.

Cada fase concluye con un informe que incluya: fase; objetivos; arquitectura; archivos creados/modificados; características realmente implementadas; comandos de compilación y pruebas y sus resultados; memoria; rendimiento; revisión de seguridad; limitaciones; pendientes y siguiente fase. Resultados fallidos o verificaciones no ejecutadas deben quedar explícitos.

Cada fase debe cerrar explícitamente las decisiones técnicas y riesgos que le corresponden; los subapartados de pruebas y puertas de salida son obligatorios, al igual que el mapa de módulos/archivos. No iniciar dependencias posteriores antes de cumplir su puerta.

| Fase | Dependencia interna | Decisiones y riesgos que deben resolverse | Validación/aceptación |
|---|---|---|---|
| F1 | Ninguna | Contratos de G-M-P, ownership, TLS/afinidad, stacks/slabs/pools, ABI/unwind, I/O y cero dependencias | Auditoría y 14 documentos/6 ADR coherentes; riesgos críticos resueltos o acotados con coste, límite y prueba; MSRV/dependencias comprobados; no se implementa runtime |
| F2 | F1 | Identidad/estados de G; layout de stack, reserva individual vs slab, protección, reutilización y límites | Transiciones válidas; ownership y stacks estables/protegidos sin liberación prematura |
| F3 | F1-F2 | ABI/registros/contexto inicial; estrategia de panic sin unwind cruzando context switch | Cambio y retorno repetidos, alineación/integridad de stack y panic bajo el contrato elegido |
| F4 | F1-F3 | Relación M-P, pool de workers, afinidad, TLS y parada/errores de thread | Ciclo M-P y shutdown correctos con número de M acotado |
| F5 | F1-F4 | Políticas de colas, overflow, robo, fairness y parking; baseline sincronizado | Sin ejecución duplicada, pérdida de trabajo ni busy waiting; benchmark reproducible |
| F6 | F1-F5 | Semántica de yield/bloqueo y límites de preemption/bloqueo externo | No hay wake-up perdido; una G bloqueada no ocupa M cuando usa primitivas gestionadas |
| F7 | F1-F6 | Orden/fairness, cierre/cancelación y registro/limpieza de waiters/select | Pruebas concurrentes de primitivas, canales, select, wake-up y shutdown |
| F8 | F1-F7 | Estructura de timers y resolución de carreras de expiración/cancelación | Plazos, cancelación, periodicidad y latencia medidas; no hay wake-up duplicado |
| F9 | F1-F8 | Readiness vs completion, ABI/FFI, descriptor y offload de I/O bloqueante | I/O real para sockets/pipes/archivos regulares según soporte declarado |
| F10 | F1-F9 | Pools/arenas/slabs y retención; no reemplazar allocator global sin evidencia | Reutilización/liberación seguras y mejora medida frente a baseline |
| F11 | F1-F10 | Metodología, cargas comparables y optimización guiada por perfil | Datos reproducibles; sin métricas o afirmaciones no medidas |
| F12 | F1-F11 | Cierre de invariantes, API y soporte declarado | Auditoría integrada y matriz de aceptación completa, incluida escalabilidad progresiva |
| F13 | F12 por destino | Viabilidad/ABI/OS por plataforma y separación readiness/completion | Compilación y pruebas representativas independientes por cada destino |
| F14 | F2-F7 y F12 | Seguridad de lifetime, join, panic, cancelación, nesting y shutdown | ADR y pruebas aprobados, o decisión explícita de no implementar |

## Hallazgos de auditoría incorporados

| Gravedad | Hallazgo | Tratamiento y fase |
|---|---|---|
| Crítico | El unwind no puede cruzar de forma no soportada el context switch/Assembly; `catch_unwind` no valida stack, ABI ni unwinder. | Trampoline por G y política de resultados/recursos en F1/F3; pruebas de panic en F3/F12. |
| Crítico | TLS es por thread M, no por rutina; no existe detección automática de uso incompatible en una closure arbitraria. | Política de migración, afinidad y eventuales APIs fijadas/task-local en F1; pruebas/límites en F4/F12. |
| Alto | ABI de libc y syscall del kernel pueden diferir; crear threads con syscalls propias elude inicialización necesaria de `std`. | Preferir `std`; contratos y ABI verificados por syscall nativa en F1/F3/F9; nunca crear threads con syscalls crudas. |
| Alto | Readiness y completion tienen ciclo de vida diferente; `epoll` no es solución universal para archivos regulares. | Abstracción dividida y estrategia de I/O con offload/handoff acotado en F1/F9/F13. |
| Alto | Guard page no implementa crecimiento ni recuperación de stack; reservas/commit/RSS no son equivalentes. | Decisión explícita, límites y pruebas en F1/F2/F12; no declarar growth hasta verificarlo. |
| Medio | Work stealing/colas lock-free prematuros elevan riesgo de ABA, reclamación y carreras. | Baseline con sincronización de `std`, pruebas y mediciones antes de optimizar en F5/F11. |
| Medio (configuración alineada; validar el código futuro) | Manifest/toolchain ahora fijan Rust 1.85/1.85.0; la compatibilidad de cada API/Assembly futura requiere validación al implementarse. | Mantener MSRV 1.85 en CI y revisar cada fase. |
| Medio | Árbol de carpetas/placeholders no demuestra funcionalidades. | Módulos incrementales y evidencia de ejecución/pruebas por criterio de salida en todas las fases. |

El repositorio contiene actualmente el scaffold/documentación, no el runtime: esta revisión no ejecuta ni certifica comportamientos de scheduler, stacks o I/O.

## Fase 1 — Arquitectura, fundamentos y decisiones

**Objetivo:** cerrar el diseño antes de escribir el motor de ejecución.

### 1.1. Alcance, toolchain y política de dependencias
- Confirmar el crate autónomo, su API prevista, la plataforma inicial y la prohibición absoluta de crates externos.
- Mantener `rust-version = "1.85"` y fijar `rust-toolchain.toml` a 1.85.0; compilar y probar con esa versión exacta. La compilación del scaffold no prueba APIs futuras todavía no usadas.
- Confirmar manifest de paquete `rust-gmp-runtime` 0.1.0, edición 2024, biblioteca `rust_gmp_runtime`, `src/lib.rs`, dependencias y dev-dependencias vacías.
- No versionar `Cargo.lock` para esta biblioteca sin dependencias; reconsiderarlo solo si aparece aplicación/binario/workspace de pruebas con razón reproducible concreta.
- Validar los perfiles propuestos: release `opt-level = 3`, LTO, `codegen-units = 1`, `strip = true`; dev `opt-level = 1`. Definir configuración de benchmarks sin crates.
- Definir política de panic. No fijar `panic = "abort"` universalmente; estudiar unwinding y distribución según los límites del context switching.
- Establecer cómo comprobar dependencias transitivas y lockfile sin añadir herramientas de desarrollo como dependencias del crate.
- En el manifest, verificar `[dependencies]` y `[dev-dependencies]` vacíos. Herramientas externas de análisis podrán utilizarse manualmente y reportarse, nunca añadirse como dependencias del runtime.

### 1.2. Modelo G-M-P y ciclo de vida
- Definir responsabilidades, invariantes y relaciones de G (Rutina), M (Machine/thread del SO) y P (Processor lógico).
- Diseñar estados y transiciones de G (`Created`, `Runnable`, `Running`, `Waiting`, `Syscall`, `Dead`) y estados de M/P; especificar qué transiciones requieren operaciones atómicas.
- Documentar el ciclo de ejecución, asociación/desasociación M-P y condiciones para que una G no se ejecute dos veces ni vuelva a una cola tras terminar.
- Diseñar identificadores, metadatos mínimos, finalización, bloqueo, wake-up, shutdown y la conducta ante errores/pánicos.

### 1.3. Ownership, colas y concurrencia
- Especificar quién posee G, stacks, contextos, colas, waiters y recursos nativos en cada estado.
- Diseñar interfaces de colas local, global e inyección, overflow y Work Stealing; no elegir lock-free sin invariantes demostrables.
- Definir la relación entre bloqueo de G y bloqueo de M; cuándo una M bloqueada puede liberar su P y cómo se evita el polling activo.
- Especificar migración de rutinas, estabilidad de dirección, TLS y estado dependiente del thread.
- Definir expresamente que `thread_local!`/TLS de SO pertenece a M y que destructores TLS no equivalen a destructores por G. `Send`/`'static` no detectan una dependencia semántica del TLS.
- Decidir la política de afinidad: contrato migrable por defecto y si se necesita API de tarea fijada a M. Una G fijada no debe migrarse; documentar cómo se asigna el worker, su efecto de capacidad, ownership y shutdown. No asumir que `#[inline(never)]` soluciona TLS.
- Si se requiere estado por G, evaluar una abstracción task-local propia separada de TLS. No afirmar virtualización TLS general ni compatibilidad automática con bibliotecas nativas/thread-affine.

### 1.4. Estrategia de stacks y memoria
- Comparar stacks estables con reserva de espacio virtual, compromiso progresivo de páginas, guard pages, límites y pools; documentar diferencias entre memoria virtual reservada, comprometida y residente.
- Definir alineación, crecimiento, protección ante overflow, liberación y reutilización segura.
- Rechazar stack copying/relocation no formalmente probado y definir cómo se protege un stack ante uso concurrente o liberación prematura.
- Establecer estrategia de asignación, métricas y límites configurables sin reemplazar el allocator global por anticipado.
- Comparar mapeo por stack contra slabs virtuales subdivididos y pools; modelar número de syscalls de mapeo y VMA count por separado, porque `mprotect` por slot puede dividir VMAs aunque las reservas se agrupen.
- Medir `/proc/self/maps`, el `vm.max_map_count` existente y límites de memoria virtual sin modificarlos. Documentar fragmentación, política de liberación de slabs, guard regions, reutilización y retención máxima.
- Definir métricas del Stack Manager: stacks activos/libres/reutilizados/liberados, slabs/capacidad, mapeos/VMA, virtual reservado, páginas committed/tocadas, RSS, fragmentación, metadatos/bytes por G, fallos y latencia.
- Tratar 8 KiB iniciales y 256 KiB reservados solo como puntos de benchmark. Elegir explícitamente tamaño fijo y conducta overflow o mecanismo de crecimiento; mapear guard page no implica expansión segura.
- Especificar comportamiento ante overflow y si puede recuperarse sin corromper memoria; considerar alt signal stack/handlers solo si el diseño realmente los necesita y puede demostrar seguridad.
- Incluir perfiles de uso de stack para `println!`/formateo, recursión, locals grandes y llamadas profundas; reportar memoria reservada, tocada/comprometida y residente por separado.

### 1.5. Context switching, ABI y fronteras de bajo nivel
- Definir contexto mínimo, registros preservados/volátiles, stack pointer, alineación, convención de llamada, trampoline y retorno.
- Especificar por separado ABI y registros para x86_64; identificar qué deberá cambiar para AArch64.
- Delimitar Rust seguro, módulo Assembly y FFI/OS. Verificar firmas, tipos, ABI, errores y contratos contra documentación fiable de Linux; no inventar declaraciones.
- Diseñar manejo de panic/unwinding para que nunca atraviese arbitrariamente contextos o frames Assembly; registrar política inicial y límites.
- Especificar trampoline de G que capture panic dentro de su stack con `catch_unwind` bajo `panic = "unwind"`, registre resultado, complete G y retorne al scheduler sin unwind a través de Assembly. Definir payload, doble panic, destructores y tareas dependientes.
- Diferenciar `panic = "abort"` (terminación, sin captura recuperable) de unwind; no inferir que `catch_unwind` prueba ABI, stack alternativo o unwind-safety.
- Preferir `std::thread` para crear/administrar Machines; prohibir `clone`/syscalls crudas para crear threads en el runtime Rust.
- Para cada FFI/syscall, registrar fuente normativa, libc frente a ABI kernel, número (si aplica), registros/argumentos/clobbers, retorno/error y razón de no usar `std`. Evaluar `mmap`, `mprotect`, `munmap`, `epoll_*`, `futex` y señales individualmente; no implementar todos por defecto.

### 1.6. Interfaces, arquitectura y documentación de diseño
- Documentar API interna/publicable, configuración y errores; identificar operaciones que dependen del thread y las que el runtime no puede interceptar.
- Producir diagrama G-M-P, ciclo de vida, estados, colas, ownership, interfaces, estrategia de pruebas, riesgos y arquitectura de carpetas.
- Escribir/revisar los seis ADR: GMP, stack, context switching, scheduler, memoria y abstracción de plataforma.
- Asignar criterios y casos de prueba para cada subsistema sin implementar el scheduler.

### 1.7. Netpoller, modelos de I/O y handoff
- Separar en contrato registro de descriptor, operación, evento readiness, finalización completion, asociación G, cancelación, error, wake-up y ciclo de vida.
- Diseñar readiness Linux `epoll`/macOS `kqueue` aparte de completion Windows IOCP; no ocultar sus semánticas distintas bajo un evento que sugiera operación completada.
- Definir política para sockets, pipes y archivos regulares. No asumir que `epoll` admite archivos regulares.
- Diseñar handoff/reemplazo y recuperación de P para bloqueo, estrategia de workers auxiliares acotados cuando se requiera, cancelación/shutdown y seguimiento de operaciones pendientes; no crear workers ilimitados.

### 1.8. Revisión documental, riesgos y plan de validación
- Registrar diagnóstico de riesgos críticos/altos/medios/bajos y distinguir problemas de especificación de funcionalidades que aún no existen en código.
- Completar documentos de FASE 1 enumerados en la Parte XXI de `SPEC.md`, con decisiones, alternativas, costes de CPU/memoria, riesgos, límites y validación. No crear stubs.
- Añadir a la matriz de pruebas casos de panic, TLS/afinidad, overflow, tipos de I/O, handoff de P y sincronización; una prueba de estrés no demuestra ausencia de errores de concurrencia.
- Para código futuro, incluir pruebas unitarias, integración, errores (cada variante alcanzada) y concurrencia; usar fuzz/modelo con semilla fija y reproducible y convertir hallazgos en regresiones. Anotar cuándo herramientas externas (Miri/ThreadSanitizer/Loom) son pertinentes.
- Definir benchmarks reproducibles del runtime y una línea base de threads nativos de Rust antes de optimizar: `harness = false`, `Instant`, carga equivalente, calentamiento, mínimo 10 repeticiones, igual número de workers, mediana/p99/dispersión y metadatos del entorno. F1 no produce benchmark de runtime porque no hay runtime implementado.
- Ejecutar comprobaciones del crate en MSRV 1.85.0; registrar que build/test del scaffold no prueba comportamiento ni compatibilidad de Assembly aún inexistente.

Matriz de validación prevista (diseño F1; ejecución solo en la fase indicada):

| Área | Casos requeridos | Fase(s) |
|---|---|---|
| G-M-P/ownership | Cada transición y transición ilegal; ejecución única; colas; asociación M-P; wake-up; shutdown. Modelo secuencial con semilla reproducible más concurrencia. | F2, F4-F7, F12 |
| Stack/memoria | Alineación, guardia, overflow fatal en proceso aislado, depth, `println!`, locals grandes, reserva fallida, estabilidad/liberación; RSS/virtual/VMA; cargas 1k/10k/100k/500k/1M dentro de recursos. | F2, F10, F12 |
| Contexto/panic | Ida-vuelta, registros/FP control, red zone/alineación, trampoline, panic bajo unwind, payload, destructores y build abort aislado. | F3, F12 |
| TLS/afinidad | TLS observado por M y tras migración; tareas fijadas solo si se aprobara esa API; shutdown/destructores. | F4, F12 |
| Sincronización/canales | Errores por variante, waiters uno/muchos, cancelación, carrera wake-up/parking, orden, cierre, deadlock y modelo secuencial. | F5-F8, F12 |
| Syscall/FFI | Firma/ABI/fuente; valor de retorno y errno/sentinel; punteros, rangos, fallos y liberación. Test por cada binding incluido. | F2-F3, F9, F12 |
| I/O | Readiness reintentable en sockets/pipes; archivo regular como limitación o offload aprobado; eventos duplicados, cancelación, timeout, error y shutdown. | F9, F12 |
| Portabilidad | Build + pruebas reales de stack, context switch, scheduler y backend específico por SO/arquitectura. | F13 |

**Entregables:** diagnóstico de auditoría; arquitectura y diagramas; modelo de estados/ownership; política TLS/afinidad; estrategia de stack y context switching/unwind; decisión de syscalls/FFI; modelos readiness/completion y estrategia I/O; análisis de seguridad; plan de pruebas ampliado; documentos y ADR revisados.

**Salida obligatoria:** F1 no implementa código de runtime. Ningún código de F2 hasta que todos los entregables estén completos y las decisiones críticas —en especial ABI, unwind, TLS/afinidad, stack overflow/growth, syscalls, I/O y toolchain— estén resueltas o tengan limitaciones/alternativas/costes/validación registrados. F1 solo se cierra con el informe requerido, `scripts/check.sh` verde y aprobación humana explícita conforme a `agente_rules.d/10-fases.md`. Una aprobación clara de la fase en conversación basta; el agente registra su fecha y procedencia en el informe sin autocertificarla.

## Fase 2 — Rutinas y gestión de stacks

**Objetivo:** construir las unidades de ejecución y stacks seguros, todavía sin scheduler completo.

### 2.1. Modelo de rutina
- Implementar identificador único, estado, metadatos mínimos, almacenamiento de contexto, información de planificación/bloqueo y finalización.
- Hacer cumplir transiciones válidas; impedir ejecución concurrente duplicada, reencolado de `Dead` y liberación mientras haya referencias activas.
- Probar transiciones válidas/ilegales, finalización, fallos y contención de ownership.

### 2.2. Reserva y ciclo de vida de stacks Linux x86_64
- Implementar reserva de stack de direcciones estables; integrar APIs nativas solo a través de abstracciones documentadas.
- Implementar la estrategia escogida en F1 (mapeos individuales o slabs virtuales), incluyendo subdivisión/alineación, guard regions, free list y crecimiento de slabs cuando correspondan.
- Añadir alineación y límites; implementar protección/guard pages donde sea viable y comprobar límites de stack.
- Implementar liberación segura y, solo después de validarla, reutilización/pool.
- Distinguir y reportar virtual reservado, comprometido y residente cuando pueda medirse.
- Probar stacks pequeños, overflow/guard page, reutilización y liberación, recursión, variables locales grandes y formato/`println!`; configurar un tamaño operativo con evidencia, no adoptar automáticamente 8/256 KiB.
- Si no existe un mecanismo de crecimiento completo validado, mantener tamaño fijo y documentar el fallo/overflow; no describir páginas bajo demanda como stack growing del runtime.
- Medir mapeos/VMA, cantidad de slabs, fragmentación y bytes por G; no modificar sysctl ni límites globales del SO para pasar pruebas.

### 2.3. Revisión de seguridad del subsistema
- Encapsular punteros y operaciones `unsafe`; documentar necesidad, invariantes, aliasing, propiedad, alineación y vida útil.
- Añadir pruebas de memoria, límites, agotamiento/error de reserva, liberación y reutilización; evitar pánicos con entradas/estados malformados.

**Entregables:** gestor de rutinas y gestor de stacks funcionales en la plataforma inicial, pruebas y documentación de seguridad.

**Puerta:** stacks con dirección estable, ownership verificable y ninguna liberación prematura antes de F3.

## Fase 3 — Context Switching

**Objetivo:** guardar/restaurar contexto y cambiar entre stack del worker y stack de rutina en Linux x86_64.

### 3.1. Contrato e implementación x86_64
- Implementar la abstracción interna de contexto y la rutina Assembly mínima para guardar/restaurar registros requeridos y cambiar stack pointer.
- Crear el contexto inicial de G/trampoline y retorno al scheduler con ABI, alineación y registros documentados.
- Mantener Assembly aislado y no permitir transferencia de control sin actualizar el estado de la rutina.

### 3.2. Panic, unwinding y frontera segura
- Implementar el trampoline por G y `catch_unwind` bajo unwind, registrar éxito/panic/fallo de runtime y volver al scheduler sin cruzar el límite Assembly.
- Definir y probar disposición del panic payload, destructores, recursos y política de tareas dependientes; documentar doble panic.
- Probar/configurar `panic = "abort"` por separado, sin afirmar captura ni continuidad del proceso.
- Probar que unwinding no cruce contextos ni frames Assembly de forma no soportada; documentar destrucción de valores y limitaciones.

### 3.3. Verificación y línea base
- Probar cambios repetidos, retorno, registros preservados, alineación, integridad de stacks y coexistencia con llamadas Rust de prueba.
- Ejecutar pruebas de contexto y de seguridad; obtener benchmark inicial de cambio de contexto reproducible.

**Entregables:** context switching real x86_64, contrato ABI, inventario de `unsafe`, pruebas y primera medición.

**Puerta:** no avanzar con corrupción de stack/contexto, retorno incorrecto, ABI no verificada o errores de seguridad.

## Fase 4 — Machines y Processors

**Objetivo:** ejecutar rutinas sobre un conjunto limitado de threads nativos y asociar Machines con Processors.

### 4.1. Processor
- Crear P con cola local y estados; hacer configurable la cantidad y definir el valor por defecto según CPUs lógicas si puede determinarse fiablemente.
- Definir propiedad temporal del P y reglas de asociación con M.

### 4.2. Machine y worker
- Crear/registrar workers M con `std::thread`; implementar ciclo de vida, asociación/desasociación M-P, parada, join y manejo de errores.
- Distinguir bloqueo de G del bloqueo del thread M; diseñar liberación del P en operaciones bloqueantes y threads auxiliares cuando proceda.
- Registrar TLS y operaciones dependientes de thread; no asumir afinidad de una G a un thread.
- Definir la implementación real de afinidad si F1 la aprobó; probar que G fijada no migra y que tareas migrables observan TLS por M. `std::thread` es la vía de administración de threads; no usar syscalls crudas de creación.

### 4.3. Ciclo mínimo de ejecución
- Integrar el dispatch de una G con el context switching, con estados consistentes antes y después de transferir control.
- Probar ejecución simultánea acotada, asociación M-P, apagado y errores de inicio/parada.

**Entregables:** Machine Manager, Processor Manager y pruebas concurrentes del ciclo de ejecución.

**Puerta:** número de M no proporcional al número de G; asociación y apagado sin pérdida, doble ejecución ni bloqueo oculto.

## Fase 5 — Scheduler G-M-P

**Objetivo:** asignar, distribuir y despachar trabajo sin busy waiting.

### 5.1. Colas y publicación de trabajo
- Implementar primero Local Run Queue por P, Global Run Queue y cola de inyección con `Mutex`/sincronización convencional de `std`; definir política de overflow.
- Reducir contención global preservando invariantes de propiedad y orden definidos; no adoptar lock-free sin prueba.
- No mantener locks al ceder/suspender una G ni durante I/O; probar wake-up, ownership, carreras y liberación bajo contención.

### 5.2. Selección, dispatch y balanceo
- Implementar scheduler loop, selección de runnable, transición a `Running`, retorno por fin/yield/bloqueo y actualización de estados.
- Buscar trabajo global cuando falte trabajo local; implementar balanceo y robo de cantidad controlada entre P, con sincronización segura.

### 5.3. Parking, wake-up y métricas iniciales
- Implementar parking/unparking de M con notificación eficiente y sin polling activo.
- Evitar despertares perdidos, competencia constante por colas vacías y sleeps artificiales.
- Medir tamaño de colas, tareas por P, actividad y transiciones necesarias para diagnóstico.

### 5.4. Pruebas y benchmark
- Probar concurrencia, publicación desde otros contextos, Work Stealing, balanceo, colas vacías/overflow, fairness básica y shutdown.
- Medir throughput y latencia de planificación con workloads reproducibles; no declarar ventaja todavía.

**Entregables:** Scheduler funcional G-M-P, pruebas de carga y benchmark inicial.

**Puerta:** ninguna G se pierde, duplica o queda ejecutable en dos P; workers duermen cuando no hay trabajo.

## Fase 6 — Yield, espera, wake-up y preemption

**Objetivo:** permitir suspensión/reanudación administrada y evitar monopolización en los límites soportados.

### 6.1. Yield y estado Waiting
- Implementar `yield_now`, transición controlada de `Running` a `Runnable`, y `Waiting` con registro y reanudación.
- Diseñar wake-up exactamente una vez, cancelación, carrera entre despertar y timeout y mantenimiento de la cola.

### 6.2. Bloqueo gestionado y llamadas externas
- Hacer que las primitivas propias suspendan G y liberen P sin bloquear innecesariamente M.
- Documentar que una función síncrona de `std` que bloquea no se suspende mágicamente; determinar estrategia específica de syscall, threads auxiliares o adaptadores.
- Probar bloqueos, desbloqueos, operaciones durante shutdown y errores.

### 6.3. Preemption progresiva
- Primero implementar preemption cooperativa y puntos seguros con invariantes explícitas; probar fairness y latencia de cesión.
- Analizar preemption asíncrona solo después, por plataforma/arquitectura, con riesgos y pruebas; no llamarla implementada hasta que realmente interrumpa en condiciones seguras.
- Documentar operaciones que retrasan preemption; no usar `sleep` artificial.

**Entregables:** rutinas suspendibles/reanudables, pruebas de fairness y documentación clara de límites de preemption.

## Fase 7 — Sincronización, canales y select

**Objetivo:** ofrecer primitivas propias que bloqueen G en vez de ocupar M cuando esperan.

### 7.1. Primitivas de sincronización
- Implementar Mutex, RWMutex, Semaphore, WaitGroup y Condition Variable en la ruta compatible con scheduler.
- Definir liberación/despertar, orden de espera, cancelación, errores, atomics y condiciones de carrera.
- No usar spinlocks por defecto; exigir evidencia de benchmarks antes de cualquier uso.

### 7.2. Canales no buffered y buffered
- Implementar ownership y estados del canal, send/receive, productores/consumidores y waiters.
- Sin buffer: rendezvous emisor/receptor y espera del emisor cuando no hay receptor.
- Con buffer: buffer circular eficiente, sin reasignaciones innecesarias.
- Implementar close, TrySend y TryReceive; definir rigurosamente cierre concurrente, operaciones posteriores y liberación.
- Suspender/reanudar G correctamente en todas las esperas.

### 7.3. Select
- Añadir espera sobre múltiples canales, recepción, envío, timeout y operaciones disponibles.
- Evitar registros duplicados y wake-up múltiple por una sola espera; definir limpieza de registros, carrera con cancelación y reglas de equidad/selección.

### 7.4. Validación concurrente
- Unit/integration/concurrency tests para primitivas, buffer, canales unbuffered, cierre, contendientes, cancelación, select y shutdown.
- Buscar interleavings de wake-up y operaciones simultáneas; ejecutar stress tests sin ocultar fallos.

**Entregables:** sistema funcional de sincronización, canales y select según contratos documentados.

**Puerta:** no hay despertares perdidos/duplicados, espera que bloquee indebidamente M ni acceso/liberación concurrente inválida.

## Fase 8 — Timer System

**Objetivo:** gestionar tiempo y despertar G sin un thread por timer.

### 8.1. Diseño y estructura
- Comparar binary heap, timer wheel y estructura jerárquica mediante costes y patrones previstos; registrar decisión antes de optimizar.
- Implementar Timer Manager centralizado con cola y asociación con scheduler.

### 8.2. API y concurrencia
- Implementar Sleep, timeout, deadline, temporizadores periódicos, cancelación y wake-up.
- Suspender G en Sleep sin ocupar worker; resolver carreras entre vencimiento, cancelación, cierre y wake-up.
- Integrar timeout con `select` y sincronización sin doble notificación.

### 8.3. Pruebas y medidas
- Probar expiración, tolerancias, cancelación, periodicidad, carga simultánea, apagado y carreras.
- Medir latencia, precisión observada, coste y memoria; no prometer precisión no medida.

**Entregables:** Timer System probado y benchmark reproducible.

## Fase 9 — Netpoller Linux

**Objetivo:** multiplexar I/O de red no bloqueante mediante `epoll` y FFI nativo.

### 9.1. Contrato de polling y FFI
- Definir modelo readiness separado del modelo completion: registro/operación, evento, finalización, asociación con G, cancelación, error, wake-up y ciclo de vida.
- Verificar ABI, tipos, firmas, flags y reglas de vida útil con referencias de Linux; aislar FFI y documentar cada llamada.
- Distinguir sockets/red de archivos y operaciones que Linux no multiplexa de la misma forma.
- Preferir `std` para threads/parking cuando sea suficiente; evaluar cada syscall (`epoll_*`, `futex`, memoria, señales) y registrar ABI libc/kernel, número, argumentos, clobbers y errores solo para las que se decida usar.

### 9.2. Implementación Linux
- Implementar backend `epoll`, registro/desregistro, loop de eventos y notificación de rutinas bloqueadas.
- Integrar timers/deadlines y cancelación, evitando polling activo.
- Liberar P mientras una G espera I/O; no confundir espera de G con M bloqueada.
- Tratar sockets y pipes readiness por separado de archivos regulares; epoll no será descrito como I/O asíncrono universal.
- Para operaciones de archivos que puedan bloquear, implementar/medir handoff y recuperación de P o offload con capacidad acotada; registrar operación pendiente, cancelación, error y shutdown. No crear un thread por operación.

### 9.3. Pruebas
- Probar registro, eliminación, readiness, error/cierre, timeout, múltiples conexiones, wake-up y shutdown.
- Usar I/O real en integración; documentar condiciones del entorno y las limitaciones de operaciones de archivos.
- Cubrir sockets, pipes y archivos regulares, bloqueos, cancelaciones y errores nativos; verificar que una operación bloqueante no paralice los demás Ps.

**Entregables:** Netpoller Linux funcional, pruebas de I/O y documentación de FFI.

**Puerta:** no declarar soporte de I/O generalizado más allá de los descriptores/casos probados.

## Fase 10 — Memory Manager

**Objetivo:** reducir costes medidos sin sacrificar ownership ni retener memoria innecesariamente.

### 10.1. Línea base y métricas
- Instrumentar asignaciones/liberaciones, fragmentación observable, memoria retenida y localidad con mediciones reproducibles.
- Diferenciar memoria virtual reservada, comprometida y residente; identificar mecanismos disponibles para medir RSS en la plataforma.

### 10.2. Reutilización y allocators internos
- Implementar slab para objetos pequeños/fijos solo donde el perfil lo justifique.
- Implementar arena para ciclos de vida comunes solo donde facilite liberación segura.
- Añadir object pools y stack pools con límites y retorno/liberación deterministas.
- No reemplazar allocator global de Rust sin justificación técnica, análisis de riesgos y benchmarks comparativos.
- No implementar GC; utilizar ownership, borrowing y RAII.

### 10.3. Pruebas y aceptación
- Probar límites, agotamiento, reutilización, liberación, concurrencia, fragmentación/retención y presión de memoria.
- Comparar con línea base antes de mantener cada optimización; eliminar complejidad que no demuestre beneficio.

**Entregables:** Memory Manager interno, métricas y comparativas de memoria.

## Fase 11 — Benchmarks y optimización basada en evidencia

**Objetivo:** detectar y mejorar cuellos de botella reales; no inventar objetivos ni resultados.

### 11.1. Metodología reproducible
- Definir hardware/OS/toolchain, comandos, calentamiento, repeticiones, variación, cargas equivalentes y datos crudos.
- Registrar diferencias semánticas y condiciones; no comparar cargas no equivalentes.

### 11.2. Matriz de comparación
- Comparar el runtime propio con threads nativos de Rust en cargas equivalentes:
  A) CPU pequeñas; B) espera; C) canales; D) mixtas; E) miles de tareas; F) red; G) carga sostenida.
- Medir spawn, ejecución, latencia de planificación, context switch, RAM por tarea/total, RSS, memoria virtual, CPU, throughput, escalabilidad, espera y contención.
- Nunca inventar resultados ni afirmar mejoras de rendimiento sin evidencia reproducible.

### 11.3. Métricas operativas completas
- Rutinas creadas/activas/bloqueadas/terminadas.
- Machines y Processors activos; tareas por P; tamaños de colas.
- Cambios de contexto, tiempo esperando y planificando.
- Memoria reservada/comprometida/residente si puede medirse.
- Asignaciones/liberaciones, CPU y throughput.

### 11.4. Optimizaciones
- Optimizar scheduler/colas, context switching, memoria, balanceo y contención según perfil.
- Revisar localidad, false sharing, asignaciones, copias, atomics, switching y consumo CPU.
- Evitar busy waiting y sincronización global innecesaria; volver a medir con el mismo protocolo.

**Entregables:** benchmarks ejecutables sin crates externos, resultados trazables e informe de optimización; no cifras objetivo ficticias.

## Fase 12 — Estabilización y aceptación Linux x86_64

**Objetivo:** revisar el sistema integrado y comprobar los criterios de runtime terminado.

### 12.1. Auditoría técnica
- Revisar invariantes de estado/colas, ciclo de vida, errores, shutdown y operaciones concurrentes durante shutdown.
- Revisar cada `unsafe`, FFI, ABI, puntero, alineación, ownership, vida útil, unwinding, TLS y migración entre Machines.
- Confirmar que no hay pánicos ante estados/entradas malformadas donde deban gestionarse errores.
- Verificar cero dependencias externas (también transitivas), toolchain y build reproducible; confirmar que `Cargo.lock` no se versiona salvo que exista una justificación concreta y aprobada.

### 12.2. Suite de pruebas
- Mantener unit, integration, stress, concurrency, memory, scheduler, stack, context-switch, channel, synchronization, timer e I/O tests.
- Escenarios concurrentes: creación/finalización simultánea, bloqueo/wake-up, Work Stealing, contención, cierre/cancelación, shutdown, errores/pánicos y presión de memoria.
- Añadir pruebas dirigidas de panic/unwind/abort y continuidad del scheduler; TLS por M, migración, afinidad y recursos fijados; stacks pequeños/overflow/guard/reutilización; sockets/pipes/archivos regulares y handoff; wake-up de múltiples consumidores y deadlocks. No inferir ausencia de bugs concurrentes solo por pasar stress.
- Ejecutar escalas progresivas de 1.000, 10.000, 100.000 y 500.000; intentar 1.000.000 solo si los límites del entorno lo permiten. Para cada escala registrar RAM/RSS, memoria virtual, CPU, VMA/mapeos, tiempo de creación, planificación y finalización, y fallos de asignación. Informar el tamaño realmente probado y los límites existentes de `vm.max_map_count` y memoria virtual; no modificar parámetros globales del SO. Crear un millón de estructuras no demuestra un millón de rutinas ejecutables.

### 12.3. API, documentación y ejemplos
- Cerrar API idiomática: `Runtime::new`, `Runtime::builder`, `Runtime::spawn`, `Runtime::wait`, `Runtime::shutdown`, `spawn`, `yield_now`, `sleep`, `timeout`, `channel`, `send`, `recv`, `select`, `mutex`, `rwmutex`, `semaphore`, `wait_group`.
- Definir `Send`/`'static` cuando proceda y firmas compatibles con ownership; documentar errores, cierre, panic y garantías.
- No prometer suspensión automática de cualquier función bloqueante de `std`; documentar estrategia para syscalls, threads auxiliares y adaptadores.
- Completar ejemplos ejecutables de uso básico, muchas rutinas, canales, sincronización, timers y networking; documentar arquitectura, modelo G-M-P, scheduler, execution engine, context switching, stacks, memoria, sincronización, canales, timers, networking, plataformas, seguridad, rendimiento y metodología benchmark.

### 12.4. Puerta de aceptación
- Confirmar ejecución stackful real, contexto individual, suspensión/reanudación y migración segura probada.
- Confirmar GMP, colas locales/globales, Work Stealing, balanceo, parking/unparking; stacks protegidos/estables y liberación segura.
- Confirmar canales/select y primitivas de sincronización especificadas, timers y Netpoller Linux.
- Confirmar pruebas de estrés/concurrencia, ausencia de corrupción de stacks y ausencia de data races conocidas; describir alcance y herramientas utilizadas, sin declarar una prueba que no se ejecutó.
- Confirmar métricas y benchmarks reproducibles, seguridad/límites documentados y reportes completos.
- Generar informe final con limitaciones, trabajo pendiente y plataformas realmente compiladas/probadas.

**Entregable:** versión estable solo para el alcance validado en Linux x86_64. Cualquier criterio pendiente impide declarar terminado el runtime.

## Fase 13 — Portabilidad progresiva posterior

Esta fase empieza únicamente tras estabilizar el backend inicial y mantiene interfaces independientes de SO/arquitectura. Cada destino es un subproyecto con revisión de ABI, compilación y pruebas propias; no hereda soporte por compartir código.

### 13.1. Linux AArch64
- Implementar context switching/Assembly y ABI AArch64; validar registros, alineación, stack, memoria virtual, protección, threads y polling.
- Compilar y ejecutar suite pertinente antes de anunciar compatibilidad.

### 13.2. macOS x86_64 y AArch64
- Implementar abstracciones nativas para memoria/protección, context switching, threads, wake-up y `kqueue`.
- Validar por separado cada arquitectura y sus contratos FFI, pruebas y límites.

### 13.3. Windows x86_64
- Implementar memoria/protección, threads, context switching y polling mediante IOCP; probar ABI y ciclos de vida nativos.

### 13.4. Windows AArch64 (viabilidad)
- Investigar viabilidad, requisitos de ABI y APIs antes de comprometer implementación; registrar decisión, alternativas y coste.
- Si se implementa, compilar y probar como plataforma independiente; si no, documentar explícitamente la limitación.

**Regla de salida por destino:** no afirmar compatibilidad hasta compilar y ejecutar pruebas representativas en esa plataforma, incluidos context switch, stack, scheduler y polling aplicable.

## Fase 14 — Evaluación futura de scopes (optativa)

Esta fase solo puede comenzar después de F12. No forma parte de la API inicial ni bloquea la aceptación Linux x86_64. F13 no es requisito: scopes se evalúa primero para el backend estable que ya haya superado F12.

**Objetivo:** determinar si es posible ofrecer una API de tareas con lifetimes acotados, inspirada en `std::thread::scope`, sin permitir que referencias prestadas sobrevivan al scope.

### 14.1. Contrato y prueba de seguridad
- Definir alcance de hijos, spawn anidado, join implícito, resultado/propagación de panics, cancelación, errores de spawn y comportamiento ante deadlock.
- Probar que el scope no retorna hasta que todas las rutinas hijas hayan terminado y que ninguna operación pendiente, waiter, callback, stack o referencia prestada pueda acceder a los datos después del retorno.
- Especificar cómo se limita la vida útil en la API Rust estable; revisar cada uso necesario de `unsafe` y demostrar ownership, aliasing y sincronización.

### 14.2. Decisión y validación
- Comparar una API scoped con la API inicial `Send + 'static`; registrar complejidad, restricciones de concurrencia y compatibilidad en un ADR antes de implementar.
- Construir pruebas de compilación/contrato para referencias prestadas, hijos anidados, panic, cancelación y shutdown, además de pruebas concurrentes de terminación.
- Si no puede demostrarse la garantía sin `unsafe` no probado o restricciones incompatibles con el runtime, cerrar la fase documentando que scopes no se implementan.

**Módulos y archivos esperados:** ADR nuevo en `docs/decisions/`; solo si la decisión aprueba implementación, cambios mínimos en la API de runtime y sus pruebas. No crear un módulo artificial ni placeholders.

**Dependencias internas:** runtime, ciclo de vida y shutdown de F4/F12; finalización de rutinas de F2/F3; scheduler y wake-up de F5/F6; cancelación y sincronización de F7. No iniciar hasta que esos contratos estén implementados y probados.

**Riesgos:** referencias que escapan por tareas hijas/operaciones pendientes; deadlocks al esperar; panics que interrumpan el join; complejidad `unsafe` que debilite la API. La seguridad de memoria prima sobre la disponibilidad de la característica.

**Puerta de salida:** ADR aceptado con garantías verificables y pruebas de lifetime/concurrencia; o decisión documentada de no implementar. Nunca exponer una API scoped si la terminación de todas las tareas hijas no está garantizada.

## Mapa de arquitectura y entrega gradual

La estructura de carpetas es una arquitectura objetivo, no una orden de crear archivos vacíos. Crear cada módulo al iniciar su fase; las interfaces vacías no cuentan como funcionalidad.

| Fase | Áreas y archivos objetivo |
|---|---|
| F1 | `Cargo.toml`, `rust-toolchain.toml`, `README.md`, `LICENSE-APACHE`, `LICENSE-MIT`; `SPEC.md`, `PHASES.md`, 14 documentos técnicos, `docs/phases/phase-1-report.md`, seis ADR y `docs/benchmarks/methodology.md`. Titular del aviso de copyright MIT: KEVINS YESID VEGA MARMOLEJO. No se versiona `Cargo.lock` para esta biblioteca sin dependencias. |
| F2 | `src/routine/{mod.rs,routine.rs,state.rs,id.rs,lifecycle.rs,metadata.rs}`; `src/stack/{mod.rs,stack.rs,allocator.rs,protection.rs}`. |
| F3 | `src/context/{mod.rs,context.rs,switch.rs,arch/mod.rs,arch/x86_64.rs}`; `src/ffi/{mod.rs,memory.rs,os.rs}` y `src/platform/{mod.rs,arch.rs,syscall.rs}` según contratos realmente requeridos. |
| F4 | `src/processor/{mod.rs,processor.rs,local_queue.rs,processor_pool.rs}`; `src/machine/{mod.rs,machine.rs,worker.rs,thread_manager.rs}`; `src/runtime/{mod.rs,runtime.rs,builder.rs,lifecycle.rs,shutdown.rs}`; `src/config.rs`, `src/error.rs`, `src/platform/{thread.rs,atomic.rs}`. |
| F5 | `src/scheduler/{mod.rs,scheduler.rs,dispatch.rs,balance.rs,work_stealing.rs,parking.rs}`; `src/queue/{mod.rs,global.rs,local.rs,injection.rs,overflow.rs}`. |
| F6 | `src/scheduler/preemption.rs`; integración de `src/routine/` y `src/machine/`; actualizar `docs/scheduler.md`, `docs/gmp-model.md`, `docs/security.md`. |
| F7 | `src/sync/{mod.rs,mutex.rs,rwmutex.rs,semaphore.rs,wait_group.rs,condvar.rs}`; `src/channel/{mod.rs,channel.rs,buffer.rs,sender.rs,receiver.rs,waiter.rs,select.rs}`. |
| F8 | `src/timer/{mod.rs,timer.rs,wheel.rs,timer_queue.rs}`; documentar estructura escogida en `docs/timers.md`. |
| F9 | `src/netpoll/{mod.rs,poller.rs,registration.rs,platform/mod.rs,platform/linux.rs}`; `src/platform/linux/{mod.rs,memory.rs,netpoll.rs,thread.rs}`; `src/ffi/threads.rs`; documentar en `docs/networking.md`. |
| F10 | `src/memory/{mod.rs,allocator.rs,arena.rs,slab.rs,metrics.rs}`; `src/stack/pool.rs` y `src/stack/growth.rs` cuando estén implementados y probados. |
| F11 | `src/metrics/{mod.rs,runtime_stats.rs,scheduler_stats.rs,memory_stats.rs}`; `benches/{spawn.rs,scheduler.rs,context_switch.rs,channels.rs,memory.rs,throughput.rs}`; `docs/performance.md`, `docs/benchmarks/methodology.md`. |
| F12 | `src/lib.rs`; `examples/{basic.rs,many_routines.rs,channels.rs,synchronization.rs,timers.rs,networking.rs}`; `tests/{scheduler_tests.rs,routine_tests.rs,channel_tests.rs,synchronization_tests.rs,stack_tests.rs,context_tests.rs,stress_tests.rs}`; completar documentación pública y revisión integral. |
| F13 | `src/context/arch/aarch64.rs`; `src/netpoll/platform/{macos.rs,windows.rs}`; `src/platform/{arch.rs,syscall.rs,linux/{mod.rs,memory.rs,netpoll.rs,thread.rs},macos/mod.rs,windows/mod.rs}`; documentación y pruebas específicas de cada destino. |

Cada archivo de la arquitectura objetivo debe justificarse por una responsabilidad real. Si una responsabilidad cambia, actualizar la documentación/ADR correspondiente sin duplicar módulos ni declarar capacidades no implementadas.

## Matriz de trazabilidad de la especificación

| Especificación | Cobertura principal |
|---|---|
| Parte I, §§1–3: visión, runtime M:N stackful y objetivos obligatorios | F1, F2–F10, F12; prohibiciones en reglas transversales. |
| Parte II, §§4–5: cero dependencias y tecnologías permitidas | F1.1, F3, F9, F12.1; se mantiene en todas las fases. |
| Parte III, §§6–7: G-M-P y arquitectura lógica | F1.2–F1.6, F4–F6; diagramas y documentación de F1. |
| Parte IV, §§8–11: Runtime Core, Scheduler, gestor de rutinas y Machine Managers | F1.2–F1.3, F2, F4–F6. |
| Parte V, §§12–15: stacks, context switching, FFI/Assembly y preemption | F1.4–F1.5, F2–F3, F6, F9. |
| Parte VI, §§16–18: sincronización, canales y select | F7. |
| Parte VII, §§19–20: timers y netpoller | F8–F9. |
| Parte VIII, §21: Memory Manager | F1.4, F10–F11. |
| Parte IX, §22: plataformas iniciales y futuras | F1.5, F9, F12–F13. |
| Parte X, §23: arquitectura de carpetas y responsabilidades | Mapa de arquitectura; archivos creados incrementalmente. |
| Parte XI, §24: Cargo, perfiles y política de panic | F1.1, F3.2, F12.1. |
| Parte XII, §25: modelo de seguridad Rust, `unsafe`, migración y TLS | F1.3–F1.5, F2.3, F3.2, F4.2, F12.1. |
| Parte XIII, §26: optimización y métricas obligatorias | F5.3, F10, F11. |
| Parte XIV, §27: fases de desarrollo originales | F1–F12 preservan la secuencia; F13 explicita destinos posteriores. |
| Parte XV, §28: pruebas y cargas de 1k a 1M | F2–F10 por subsistema; F12.2 para escalas y escenarios integrados. |
| Parte XVI, §29: comparación de rendimiento | F11.1–F11.4. |
| Parte XVII, §30: API pública y límites de blocking | F1.6, F6.2, F12.3. |
| Parte XVIII, §§31–33: normas, procedimiento por módulo e informe de fase | Reglas transversales y formato de salida de cada fase. |
| Parte XIX, §34: criterios de runtime terminado | F12.4; puertas previas evitan posponer defectos críticos. |
| Parte XX, §35: primera entrega exclusivamente de F1 y advertencias | Puerta de F1; F1.4–F1.6; límites documentados para TLS, bloqueos y preemption. |
| Parte XXI, §§36–45: auditoría, TLS, panic, syscalls, stacks, colas, I/O, docs y pruebas | Hallazgos de auditoría; F1.1–F1.8; F2–F5, F9, F12; nuevos documentos técnicos y ADR. |
| Parte XXII: organización, MSRV, slabs, métricas/escalabilidad y scopes futuros | F1.1, F1.4, F2.2, F10, F12.2, F14. |

## Formato del informe de cierre de fase

```text
PHASE REPORT
1. Phase:
2. Objectives:
3. Architecture:
4. Files created:
5. Files modified:
6. Features implemented:
7. Tests executed:
8. Test results:
9. Memory considerations:
10. Performance considerations:
11. Safety review:
12. Known limitations:
13. Pending work:
14. Next phase:
```