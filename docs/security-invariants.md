# Invariantes y alcance de seguridad

Este documento define un contrato de diseño; no certifica la seguridad del
scaffold ni de un runtime que aún no existe.

## Alcance del modelo de amenazas

El runtime ejecutará closures dentro del mismo proceso con los mismos
privilegios que la aplicación. No es sandbox ni aisla código hostil. Errores de
la tarea (loop infinito, bloqueo de una API síncrona, asignación excesiva,
`process::abort` o acceso a archivos/red) no se contienen por stack separado.
Los límites protegen integridad del runtime frente a fallos accidentales y
entradas/estados inválidos; no constituyen una frontera de seguridad de proceso.

Tratar como no confiables los parámetros de configuración, tamaños, descriptores
e identificadores que cruzan FFI, y estados/eventos del SO. Validar límites,
alineación, ownership, estados y retornos; no construir comandos de shell ni
generar Rust desde contenido de tareas.

## Ejecución y estados

- Una G tiene como máximo un token de ejecución y un propietario M.
- G `Waiting`, `Syscall` o `Dead` no está en una cola runnable; `Dead` no se
  reactiva.
- El cambio de estado, registro del waiter y wake-up evitan pérdida y doble
  reanudación.
- El scheduler solo retoma control tras una salida conocida del contexto.
- Shutdown no libera tareas/handles a los que el SO o backend todavía pueda
  acceder.

## Memoria y stacks

- Stack/contexto mantienen dirección y vida útil mientras cualquier
  frame/referencia/operación pueda utilizarlos.
- No liberar, reutilizar ni mover stack activo; módulos de alto nivel no reciben
  punteros crudos internos.
- Registrar alineación, límites, owner, aliasing y vida útil de cada puntero.
- Guard page no equivale a recuperación; el overflow inicial es fail-stop, no
  corrupción silenciosa ni panic recuperable.
- No mantener referencias Rust a través de suspensión si la semántica/aliasing
  no está probada.
- Reservas, tamaños, contadores y cálculo de direcciones deben usar operaciones
  comprobadas y devolver error tipado ante overflow/desbordamiento aritmético.

## Concurrencia, TLS y FFI

- No hacer yield, bloqueo o I/O manteniendo locks internos.
- Documentar memory order y reclamación de atomics; lock-free requiere revisión
  de ABA, reclamación y carreras.
- TLS pertenece a M; no prometer que migre con G.
- Cada syscall valida ABI/tipos/errores; cada puntero/rango tiene ownership y
  vida útil explícitos.
- Callbacks, operaciones pendientes y cancelación no pueden liberar recursos
  que el kernel/backend aún pueda usar.
- Panic se captura dentro de G si unwind/ABI fueron validados; nunca cruza
  arbitrariamente context switch/Assembly. `panic=abort` termina el proceso.

## Revisión `unsafe` obligatoria

Cada bloque `unsafe` necesita: motivo; invariantes; punteros, owner, vida útil,
alineación y aliasing; operación permitida; garantía del llamador; fuente oficial
para ABI; prueba dirigida y revisión humana. Solo los módulos context, stack,
ffi y platform pueden contener `unsafe`; los demás prohíben ese código.
Pruebas de stress no demuestran seguridad de memoria ni ausencia de data races.
