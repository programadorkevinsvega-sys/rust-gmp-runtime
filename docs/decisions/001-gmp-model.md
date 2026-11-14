# ADR 001: Modelo G-M-P

- Estado: Aceptado para la arquitectura; representación atómica y código quedan
  para sus fases.
- Fecha: 2026-11-14.

## Problema

Multiplexar ejecuciones ligeras stackful sobre un conjunto acotado de threads del SO.

## Alternativas y costes

Thread por rutina (un thread nativo por tarea, alto coste de memoria); pool
convencional sin contexto stackful (no satisface el objetivo); modelo G-M-P
propio (complejidad adicional de scheduler y memoria por G).

## Decisión

Conservar G (rutina stackful), M (thread nativo) y P (capacidad lógica con
cola local) como el modelo central. G migra entre M por defecto; P está asociado
a lo sumo a una M. No presentar un thread pool sin stacks/context switching
como implementación del objetivo.

G sigue los estados `Created`, `Runnable`, `Running`, `Waiting`, `Syscall` y
`Dead` con transiciones y ownership definidos en
[`gmp-model.md`](../gmp-model.md). Solo una M posee el token de ejecución. El
detalle de atomics/orderings se decide con la implementación, no se presume del
diagrama.

## Justificación y API

Permite separar unidad de trabajo, capacidad de scheduling y thread nativo. La
API inicial prevista acepta closures `FnOnce + Send + 'static` cuando
corresponda, handle de resultado y drenaje/shutdown; no `scope`, task-local ni
afinidad a M. Añade complejidad de estados, colas, handoff, sincronización y
migración; memoria por G incluye stack/contexto/metadatos y CPU incluye
dispatch/switch.

## Costes de CPU y memoria

Dispatch, cambios de contexto, transiciones y sincronización añaden overhead frente a ejecutar directamente en un thread.

## Costes de memoria

Cada G requiere stack, contexto y metadatos; M/P/colas consumen recursos
compartidos. Medir residente y virtual por separado. El número de G no fija el
número de M; el máximo de M debe ser configurable y acotado.

## Riesgos y límites

Una G puede correr en una M distinta; TLS y recursos thread-affine no migran con ella. Bloquear M no equivale a bloquear G y exige handoff explícito de P.

## Validación

Invariantes de ejecución única, asociación M-P exclusiva, bloqueo sin consumir
P, wake-up sin pérdida, shutdown y scheduler con stacks reales. Las pruebas
deben alcanzar cada transición y comparar secuencias con un modelo. El ADR no es
evidencia de implementación.
