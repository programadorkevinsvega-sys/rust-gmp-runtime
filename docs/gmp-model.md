# Modelo G-M-P

## Responsabilidades

- **G:** ejecución lógica migrable, salvo que se diseñe explícitamente una política de afinidad. Contiene estado, contexto, stack estable y resultado terminal.
- **M:** thread del sistema operativo. Su TLS y recursos thread-affine no migran con G.
- **P:** capacidad de ejecución con cola local y estado de asociación; no es un thread.

## Estados y transiciones acordados

| Estado G | Significado y titularidad | Transiciones permitidas |
|---|---|---|
| `Created` | Construida, no visible en colas; runtime controla su inicialización. | `Runnable` al publicarse con éxito; terminal por fallo de creación sin exponerla. |
| `Runnable` | Elegible para ejecución y publicada como máximo en una cola; una reclamación debe retirarla antes de correr. | `Running` al adquirir el token exclusivo; `Dead` solo por descarte antes de publicar o fallo terminal controlado. |
| `Running` | Token exclusivo en una M asociada a un P; stack/contexto solo se usan en esa ejecución. | `Runnable` al yield; `Waiting` tras registrar waiter; `Syscall` al entrar en operación bloqueante gestionada con handoff; `Dead` al completar/panic capturado. |
| `Waiting` | Operación de sincronización/I/O pendiente con waiter registrado; no ocupa cola runnable. | `Runnable` por un solo wake-up ganador; `Dead` por cancelación solo si la operación confirma que ya no retiene recursos. |
| `Syscall` | G ejecuta una operación síncrona controlada en M; no está en cola. M puede estar bloqueada y P ya fue liberado. | `Runnable` al completar y publicar resultado; `Dead` por fallo terminal gestionado. |
| `Dead` | Estado terminal; resultado/panic observable mediante el handle. | Ninguna. Stack/recurso se libera cuando no hay referencias ni operaciones pendientes. |

Las transiciones que cruzan threads requieren exclusión/atomicidad demostrable.
F1 fija estas reglas lógicas, no orderings atómicos concretos: cada operación de
transición y publicación deberá definirlos al codificarse. En particular,
`Waiting -> Runnable` tiene un único ganador y `Dead` no vuelve a ejecutarse.

## Datos implementados en F2

`Routine` mantiene el estado bajo un mutex y `ExecutionPermit` es el token
exclusivo para reclamar `Running`. El snapshot de planificación cuenta entradas
a `Runnable`, ejecuciones y yields; acumula tiempos observados en runnable y
running, y conserva la razón/hora del bloqueo actual. Las razones `wait_for`
son sincronización, I/O, timer y fuente externa; los syscalls gestionados se
registran aparte. Wake y retorno de syscall limpian el bloqueo y vuelven a
registrar la entrada runnable.

El contexto de F2 almacena el límite superior del stack y reserva un slot para
el stack pointer guardado. Este último sigue ausente: no hay registros
guardados ni contexto ejecutable hasta implementar y validar F3. Por tanto, las
rutinas actuales modelan transiciones/ownership, pero no ejecutan closures ni
prueban tiempos de espera de un scheduler real.

Estados de M: `Starting`, `Idle`, `Running` (con P), `BlockedSyscall` (sin P
cuando el handoff fue exitoso), `Stopping`, `Terminated`. Estados de P: `Idle`,
`Owned(M)` o `HandingOff`. Un P no se asocia a dos M simultáneamente; transiciones
de asociación/desasociación se serializan.

## Ownership

- Runtime posee el registro de G aceptadas y el ciclo de vida de workers.
- Scheduler/cola contiene referencias de G `Runnable`; sacar una G y adquirir su
  token ocurre antes de ejecutar.
- G posee su closure/resultados y el owner estable de su stack/contexto hasta el
  estado terminal y liberación segura.
- Waiter/operación pendiente mantiene la referencia necesaria a G/resultado
  hasta wake-up, finalización o cancelación confirmada.
- Join handle permite observar resultado; no puede mutar estado de ejecución ni
  liberar recursos aún usados.
- M posee TLS nativo y recursos locales del thread; nunca se transfieren con G.

```text
M (OS thread) <-- asociación temporal --> P (capacidad + cola local)
                                             |
                      +----------------------+-------------------------+
                      |                      |                         |
                 G Runnable             G Running          G Waiting/Syscall
```

En F4.1, la asociación de P se representa con un guard exclusivo ligado al
thread que lo adquiere; al destruirse el guard, P vuelve a estar disponible.
La identidad lógica de M y el ciclo de vida de workers se incorporan en F4.2.

## Ciclo de vida y fallos

`spawn` crea y publica G de forma transaccional: un fallo antes de publicación
devuelve error y no deja una tarea parcialmente encolada. `wait` detiene admisión
y espera las G aceptadas. Shutdown no cancela código arbitrario ni libera
waiters/handles que el SO aún usa. Un panic capturado transforma la G en terminal
y no atraviesa Assembly; ver [panic](./panic-and-unwinding.md).

## Migración, TLS y afinidad

La política inicial es G migrable por defecto, sin API de pinning ni task-local.
La migración mueve stack/contexto entre M, no el TLS del thread. `thread_local!`,
TLS nativo, destructores TLS y recursos ligados a M tienen semántica por thread.
`Send`, `'static` y `#[inline(never)]` no detectan dependencia de TLS. Ver
[TLS y afinidad](./tls-and-thread-affinity.md).
