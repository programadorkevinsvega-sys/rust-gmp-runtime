# ADR 007: Almacenamiento de contexto y metadatos de rutina

- Estado: Aceptado para F2; el layout de registros y la inicialización de
  ejecución siguen sujetos al ADR 003 y se implementan en F3.
- Fecha: 2026-11-14.

## Contexto

F2 requiere que G tenga almacenamiento asociado a su contexto, estados,
información de planificación/bloqueo y finalización. F3 es la fase que define y
verifica el contexto de CPU System V AMD64, el trampoline y el cambio real de
stack. No se debe fingir que una G puede ejecutarse antes de esa validación.

## Decisión

- Cada rutina conserva junto con su stack un almacenamiento de contexto estable
  que registra el límite superior del stack y un slot de stack pointer
  guardado, que permanece ausente hasta que un backend F3 real lo capture.
- El estado de contexto de F2 es informativo; no contiene todavía registros
  callee-saved, estado x87/MXCSR ni dirección de reanudación, y no permite
  ejecutar o reanudar la rutina.
- El ciclo de vida mantiene contadores de publicaciones, ejecuciones y yields,
  tiempo acumulado runnable/running y el bloqueo actual con razón y hora.
  `Instant` sirve para duraciones monotónicas; la API expone snapshots, no
  mutación de esos datos.
- El `wait()` existente representa sincronización; `wait_for(reason)` permite
  declarar espera por sincronización, I/O, timer o fuente externa. El syscall
  gestionado lleva razón propia. Wake y retorno de syscall limpian el bloqueo.

## Alternativas y costes

- Guardar registros CPU ya en F2 requiere un layout acoplado a ABI sin
  consumidor/validación; se descarta hasta F3.
- Usar contadores atómicos evita locks de lectura/escritura, pero complica
  snapshots coherentes y ordenamientos; se mantiene el mutex de ciclo de vida.
- Omitir tiempos/razones reduciría overhead, pero deja sin evidencia la espera
  runnable y el motivo de suspensión que scheduler y benchmarks necesitan.

## Consecuencias

La creación guarda el límite superior del stack en el contexto; el snapshot de
planificación permite observar transiciones sin exponer handles mutables. Cada
transición actualiza timestamps/contadores bajo el lock ya requerido por el
estado. No se añade asignación por rutina para el contexto.

## Riesgos

Las mediciones de duración incluyen overhead del lock y no son métricas de
tiempo de CPU. Los tiempos se acumulan con `Duration` y solo representan tiempo
observado desde las transiciones. El campo de stack pointer no equivale a un
contexto inicial ejecutable; F3 debe validar layout, ABI, alignment, red zone,
registros y unwinding según ADR 003.

## Validación

Pruebas deterministas comparan secuencias de transición con el modelo; pruebas
de integración comprueban contadores, razones, timestamps presentes/ausentes,
contexto inicialmente no reanudable y retención del stack. `scripts/check.sh`
debe pasar. Benchmark `harness = false` con al menos diez muestras contrasta
operaciones de ciclo de vida con creación de threads nativos y documenta las
diferencias semánticas; no se infiere rendimiento de ejecución stackful.
