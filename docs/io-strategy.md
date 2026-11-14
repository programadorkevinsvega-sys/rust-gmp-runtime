# Estrategia de I/O y operaciones bloqueantes

## Clasificación y decisión inicial

- **Sockets:** primera clase prevista para readiness Linux con modo no bloqueante;
  readiness exige intentar la operación y manejar `WouldBlock`.
- **Pipes:** evaluar soporte readiness y cierre por separado.
- **Archivos regulares:** fuera del Netpoller inicial; no asumir soporte de
  `epoll`, ni offload transparente.
- **Otras llamadas síncronas de `std`:** el runtime no puede suspenderlas
  automáticamente.

## Handoff para operaciones gestionadas

Solo las APIs de I/O explícitas del runtime pueden transformar una operación
controlada en `G Waiting`. Registrar operación/recurso, publicar waiter, soltar
P y permitir que otra M use el P; al readiness, intentar la operación, registrar
resultado y despertar G exactamente una vez. Al completar una syscall fuera del
Netpoller, recuperar P antes de continuar o reencolar G. Un fallo de handoff no
puede dejar G ejecutando sin P ni publicada dos veces.

Las tareas que llamen por su cuenta a `std::fs` u otra API bloqueante bloquean su
M; es una limitación explícita de la API inicial. Si se aprueba offload futuro
para archivos, usar workers auxiliares acotados con cola, backpressure, drenaje,
cancelación y shutdown definidos. No crear un thread por operación.

## Pruebas

F9: I/O real con sockets/pipes no bloqueantes; archivo regular debe probar la
limitación declarada o la estrategia de offload aprobada. Probar bloqueos,
timeouts, cancelación, errores y shutdown; comprobar handoff y recuperación de P
y que una espera gestionada no ocupe innecesariamente todos los Ps.

**Estado:** límites iniciales acordados; no prometer I/O general hasta
implementar y validar cada tipo.
