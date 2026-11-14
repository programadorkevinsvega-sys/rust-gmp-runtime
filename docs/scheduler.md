# Scheduler

## Decisión de baseline

La primera implementación usará colas convencionales sincronizadas con
`std::sync::Mutex`; no se inicia con lock-free ni spinlocks. `std::thread` y
primitivas de parking de `std` administran workers. No se usará `std::sync::Mutex`
como sustituto de una primitiva cooperativa de G: tomarlo puede bloquear M.

La línea base conceptual contempla:

- **Local run queue:** preferencia para trabajo del P; solo el owner puede
  consumir por el camino rápido.
- **Global run queue:** overflow y redistribución; acceso protegido.
- **Injection queue:** punto de entrada para `spawn` externo y workers sin P.
- **Work stealing:** una M ociosa intenta global/injection antes de robar de un
  P; roba un lote acotado y nunca ejecuta una G todavía publicada en otra cola.
- **Parking:** worker sin trabajo se duerme con mecanismo de `std`; publicación
  y notificación se coordinan para no perder wake-ups ni hacer busy waiting.
- **Handoff:** al bloquearse una M en syscall gestionada, liberar P de forma
  explícita; al retorno, la G compite por recuperar un P y solo se publica como
  runnable después de que su resultado esté disponible.

Tamaños de colas/lotes, fairness y estrategia de notificación se decidirán
durante F5 con pruebas y benchmarks. Estas decisiones no implican que el
scheduler exista actualmente.

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

Probar publicación desde varios threads, colas vacías/llenas, wake-up concurrente
con parking, stealing, fairness, shutdown y ausencia de ejecución simultánea de
G. Fuzz/modelo reproducible comparará secuencias de colas con `VecDeque`;
pruebas de stress se repetirán con semillas registradas. Usar Loom/Miri/
ThreadSanitizer solo como herramientas externas cuando aplique. Stress tests son
evidencia útil, no demostración de ausencia de carreras.
