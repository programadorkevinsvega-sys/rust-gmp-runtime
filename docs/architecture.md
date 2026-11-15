# Arquitectura general

## Alcance

Diseñar un runtime propio M:N stackful con modelo G-M-P. Este documento describe límites y decisiones iniciales; no afirma que el runtime esté implementado. La plataforma objetivo inicial es Linux x86_64 y el proyecto no añadirá crates externos.

## Archivos principales

```text
rust-gmp-runtime/
├── Cargo.toml
├── LICENSE-APACHE
├── LICENSE-MIT
├── README.md
├── SPEC.md
├── PHASES.md
├── docs/
│   └── phases/phase-1-report.md
└── src/
```

## API prevista y límites

La API inicial prevista es un `Runtime` configurable, `spawn` para closures
`FnOnce + Send + 'static`, un handle de finalización/resultado y una operación
de drenaje/shutdown. `spawn` debe informar el rechazo si el runtime ya no acepta
trabajo. El runtime conserva ownership de las tareas aceptadas hasta que
terminen; el handle observa el resultado y no controla directamente el stack.
La firma pública y los tipos concretos de error se fijarán al implementar la
API. No se ofrece `scope`, captura de referencias prestadas, semántica task-local
ni afinidad a M en la API inicial.

El runtime no intercepta automáticamente llamadas bloqueantes de Rust ni puede
detectar el uso de TLS o recursos ligados a un thread dentro de una closure.
Solo las operaciones que exponga explícitamente como cooperativas podrán
suspender G y realizar handoff de P.

## Componentes y ownership

- **G (rutina):** unidad de ejecución, estado, closure/resultado, contexto y
  stack de dirección estable. La transición exclusiva a `Running` da a una
  sola M derecho de ejecución.
- **M (machine):** thread del sistema operativo creado y administrado con
  `std::thread`. Ejecuta G, conserva su propio TLS y nunca transfiere ese TLS a
  otra M. El manager registra cada worker en su TLS, conserva su P mientras
  espera y despacha trabajos one-shot a la cola local de ese P. No hay yield ni
  reanudación de G.
- **P (processor):** capacidad lógica de ejecución con una cola local protegida.
  Pertenece a lo sumo a una M; durante handoff puede quedar disponible para
  otra M. La implementación inicial usa un binding exclusivo y temporal al
  thread actual, y dimensiona el pool por `available_parallelism` cuando se
  solicita el valor predeterminado.
- **Scheduler:** coordina colas, transiciones, asociación M-P, parking,
  notificaciones y drenaje.
- **Execution engine:** frontera aislada de contexto, stack y Assembly; retorna
  al scheduler por trampolines controlados.
- **Platform:** APIs del sistema operativo aisladas tras contratos por
  operación; usar FFI solo cuando `std` no provea lo requerido.

El detalle de las transiciones y la titularidad por estado está en
[G-M-P](./gmp-model.md). Los objetos compartidos pueden tener varios handles,
pero solo un token de ejecución permite que G corra. Ningún handle de usuario
puede liberar un stack activo.

F1 no aprobó pinning de G. Los workers ejecutan dispatch one-shot y conservan
su asociación a P mientras esperan; no interceptan llamadas bloqueantes ni
hacen offload transparente.

```text
 Runtime/API
     |
 Scheduler -- Global queue / injection queue
     |
   M <-> P -- Local run queue
     |          |
     +---- G ---+
          | stack/context
          v
  Execution engine
     |
  Platform: memory, readiness/completion I/O, wake-up
```

## Invariantes de ejecución

1. Solo una M posee el token de ejecución de una G en un instante.
2. Solo G en estado `Runnable` puede publicarse en una cola ejecutable; `Waiting`,
   `Syscall` y `Dead` no son runnable.
3. La transición a `Waiting` y el registro del waiter se serializan respecto al
   wake-up para evitar pérdida o duplicación.
4. El stack y contexto conservan direcciones estables mientras cualquier frame,
   referencia o operación pendiente pueda utilizarlos.
5. Una operación que bloquea M no implica que G esté suspendida. El handoff de
   P solo ocurre en rutas controladas por el runtime.
6. G migrable no conserva TLS de M. `thread_local!` no es task-local.
7. El scheduler solo retoma G tras un retorno conocido del context switch y
   actualización de estado fuera del stack de G.
8. La publicación y la transición de estado forman una única operación lógica;
   los orderings atómicos se decidirán y revisarán junto con cada implementación.
9. Shutdown deja de aceptar trabajo, drena las G aceptadas y no libera recursos
   todavía referenciados por una operación del SO.

## Configuración, errores y shutdown

El builder validará límites antes de crear workers o reservar memoria. Los
fallos de creación de worker, reserva/protección de stack, envío de tarea,
operación de I/O y shutdown deben conservar operación y causa en errores
tipados. Los fallos no pueden convertirse en éxito vacío ni ocultarse.

La llamada de drenaje cierra la admisión de nuevas G y espera todas las ya
aceptadas; una operación de shutdown explícita debe ser idempotente y reportar
fallos de unión/limpieza. No prometer cancelación forzada de código Rust
arbitrario.

## Secuencia de vida

```text
RuntimeBuilder -> Runtime (workers M + processors P)
spawn -> G Created -> G Runnable -> cola
cola -> asociación M-P -> G Running
G Running -> Runnable (yield) | Waiting (primitiva del runtime)
           -> Syscall (handoff controlado) | Dead (resultado terminal)
shutdown -> cerrar admisión -> drenar G/operaciones -> unir M -> liberar recursos
```

El diagrama es un contrato de diseño, no evidencia de implementación. Las
pruebas y restricciones por fase están en [PHASES.md](../PHASES.md).

Ver [GMP model](./gmp-model.md), [scheduler](./scheduler.md), [context switching](./context-switching.md) y [security invariants](./security-invariants.md).
