# Scheduler

## Baseline implementada en F5.1–F5.4

El scheduler usa un `Mutex` convencional para proteger conjuntamente una run
queue local por P, la global y la de inyección. Una `Condvar` comparte ese mismo
mutex para el parking de M; no hay polling activo, sleeps artificiales ni colas
lock-free. Cada cola local tiene capacidad de 256 tareas; transferencia y robo
están limitados a 16 tareas por operación. La cola de inyección admite 1.024
tareas y, al llenarse, las publicaciones pasan a la global, que reserva memoria
antes de aceptar el trabajo. Si falla la reserva, la publicación se rechaza y el
dispatch revierte la reserva de lifecycle.

La publicación inserta el trabajo bajo el lock y luego notifica a un worker que
ya está aparcado. Como el worker revisa las colas y entra en `Condvar::wait`
mientras mantiene el mismo lock, una publicación que coincide con la transición
a parking no puede perderse. Las notificaciones se limitan a workers aparcados
que todavía no tienen una notificación pendiente. El cierre marca el scheduler
cerrado y notifica a todos; los workers drenan el trabajo aceptado y salen cuando
las colas quedan vacías.

`MachineManager::dispatch` asigna un Machine de origen en round-robin, pero
publica cada tarea en la cola de inyección; no la inserta directamente en la
cola local de ese Machine. Al seleccionar, un P consulta primero su cola local,
después repone hasta 16 tareas desde la global o desde la inyección, y solo
cuando ambas están vacías intenta robar de otro P. El robo mueve como máximo la
mitad del trabajo de la víctima, con techo de 16 y mínimo de una tarea cuando
queda trabajo local. Por eso una ráfaga puede ser consumida directamente desde
la inyección por varios P antes de que se necesite robar. La sección crítica
cubre solo selección, transferencia y publicación; nunca la ejecución del
closure ni el cambio de contexto. Las tres colas comparten un lock, por lo que
esta versión prioriza invariantes auditables sobre contención local. La
selección puede ejecutar una tarea en un M distinto del que originó la
publicación.

`MachineManager::scheduler_stats` devuelve snapshots de ocupación actual y
picos por cola, workers aparcados, eventos de parking y notificación, tareas
publicadas/seleccionadas/completadas, selecciones por P, intentos de robo,
intentos fallidos por no hallar trabajo local transferible, tareas robadas,
rechazos y transiciones de estado de M. Las longitudes son instantáneas y los
contadores son acumulativos desde el arranque. El snapshot toma el lock del
scheduler y se debe consultar de forma diagnóstica, no desde cada tarea.

La ejecución sigue siendo one-shot: solo se implementa el retorno al terminar;
yield, suspensión y reanudación stackful pertenecen a F6. Si un worker falla y
deja tareas pendientes, shutdown las rechaza explícitamente y resuelve sus
handles en vez de dejarlos esperando indefinidamente.

## Asignación optativa de stacks en el worker

`MachineManager::dispatch` conserva la API existente para una `Routine` que ya
tiene stack. Para que la asignación sea local al Processor real, se añadieron
`start_with_stack_manager` y `dispatch_new`: el identificador se reserva al
publicar, pero el worker que finalmente selecciona la tarea crea la `Routine`
y su stack. `start_with_stack_manager` usa el pool global; la variante
`start_with_processor_caches` exige el motor Go-inspired y crea una caché
exclusiva dentro de cada worker después de enlazar su Processor. La rutina
devuelve el lease a esa misma caché solo después de completar y invalidar su
contexto. Si el scheduler roba la tarea, la caché pertenece al worker que la
ejecuta, no al Machine round-robin de origen.

Esta ruta es optativa: `MachineManager::start` y `dispatch` no cambian de
política ni de default. Las cachés se limitan según `GoInspiredOptions` y se
vacían al apagar los workers. El estado M `Preparing` distingue la selección y
adquisición del stack de la ejecución efectiva de G. La comparación completa
de `dispatch_new`, incluyendo publicación, ejecución y join, está en
[`A/B scheduler con caché por Processor`](./benchmarks/results/2026-11-15-machine-processor-cache.md).

## Reglas de sincronización

- No mantener locks mientras una G hace yield, se suspende, ejecuta una operación bloqueante o cambia de contexto.
- Coordinar transición de estado y publicación en cola para impedir doble ejecución, pérdida o wake-up perdido.
- No confundir bloqueo de G con bloqueo de M; al bloquear M, realizar handoff y recuperar P mediante una ruta explícita.
- Medir contención/latencia antes de reemplazar `Mutex`. Chase-Lev u otra cola lock-free requiere revisión de ownership, ABA, órdenes de memoria y reclamación.
- Separar una G Ready para ejecución de operaciones pendientes de I/O que todavía no se han completado.
- La cola no es dueña de la finalización: la transición de estado autoriza la
  ejecución; un elemento obsoleto o duplicado se rechaza sin ejecutar G dos
  veces.
- El cierre de admisión, drenaje y shutdown son estados coordinados del Runtime,
  no una señal informal a los workers.

## Validación requerida

La cobertura incluye consumo de una ráfaga y retorno a parking con un worker,
publicación con workers aparcados, ejecución concurrente en todos los workers
ociosos, dispatch exactly-once bajo stress, selección por cada P en las cargas
prolongadas, robo desde una cola local saturada a un worker inactivo, overflow,
rechazo tras cierre, métricas y drenaje en shutdown. El test de robo llena
deliberadamente la cola local de la víctima y ejecuta la selección del ladrón
por la ruta bloqueante `wait_next`; verifica IDs únicos, contadores y propiedad
de todas las rutinas.
`cargo bench --bench scheduler` separa batches de throughput sin temporizador ni
contador por rutina, batches de latencia instrumentada y muestras de una tarea
publicada cuando todos los workers están aparcados. Los campos submit y drain
se miden por separado. Publicación-a-entrada sigue siendo una medición compuesta
de reserva, publicación, despertar (si aplica), planificación del hilo,
selección y entrada en el closure; no atribuye tiempo puro a cada etapa.
Resultados, hardware, límites y salida cruda se registran en el informe de
caracterización correspondiente. No son SLO ni prueban una ventaja sobre otra
política o escalabilidad general. Stress tests son evidencia útil, no
demostración formal de ausencia de carreras.

`cargo bench --bench scheduler_isolated` ejecuta exactamente un manager en cada
proceso y admite `RUST_GMP_SCHEDULER_WORKERS`, `RUST_GMP_SCHEDULER_SAMPLES` y
`RUST_GMP_SCHEDULER_TASK_COUNTS`. Compara ráfaga (publica todo el lote mientras
las entradas esperan en una compuerta, y luego las libera para ejecutar) con
flujo (las entradas empiezan mientras el caller sigue publicando). No equivale
a un benchmark del selector aislado: ambos intervalos contienen dispatch y
join. La prueba determinista de cola local saturada complementa esos escenarios
para forzar desequilibrio y comprobar robo; el API público actual publica en la
cola de inyección y no permite fijar directamente el destino P de cada rutina.
Las métricas de ocupación máxima son acumulativas desde el inicio del manager;
intentos/éxitos de robo y tareas por P se reportan como deltas por batch.
