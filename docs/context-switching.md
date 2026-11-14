# Context Switching

## Alcance

El cambio de contexto stackful es propio y específico de arquitectura. La
implementación inicial está limitada a Linux x86_64. F3.1 añade una ejecución
de una sola entrada con retorno al stack del llamador; todavía no hay yield,
reanudación planificada, migración entre workers ni scheduler integrado.

## Contrato acordado para Linux x86_64

El backend Linux x86_64 bajo System V AMD64 ABI conserva `rsp`, la continuación
y `rbx`, `rbp`, `r12`–`r15`. El layout `repr(C)` tiene offsets verificados en
compilación. El cambio inicializa el stack con `rsp % 16 == 8` en la entrada al
trampoline, reserva el red zone de 128 bytes y limpia el Direction Flag antes
de transferir. Cada contexto conserva el control word x87 y los bits de control
MXCSR exigidos por la ABI; no persiste registros vectoriales caller-saved.

El trampoline inicial recibe un frame interno en `r12`, llama a la entrada C y
retorna al contexto del scheduler. `Routine::run_once` adquiere primero el
`ExecutionPermit` (estado `Running`); al terminar la entrada, el trampoline
marca la rutina `Dead` antes del cambio de vuelta. Una inconsistencia que
impida esa transición es fail-stop. La API aún no permite pausar/reanudar G.

El estado inicial entra por un trampoline controlado, nunca por una dirección
proporcionada por el usuario como retorno. Esta implementación solo transfiere
con G en estado `Running` y el token exclusivo en poder del llamador; el
trampoline completa la transición a `Dead` antes de volver. Contexto y stack
permanecen estables durante la ejecución. El contexto no guarda ni restaura TLS;
la política de migración de G es independiente del estado TLS de M.

Fuentes: [System V AMD64 ABI (proyecto x86-psABIs)](https://gitlab.com/x86-psABIs/x86-64-ABI)
y [Intel 64 and IA-32 Architectures Software Developer Manuals](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).
El cambio de contexto de bajo nivel permanece encapsulado en
`src/context/arch/x86_64.rs`; no asumir que x86_64 y AArch64 comparten
registros o ABI.

## Panic

En F3.1 la entrada usa `extern "C"`: unwind a través de esa frontera no está
soportado y no se captura. La política de panic, `catch_unwind`, destructores y
validación del unwinder pertenecen a F3.2. `panic = "abort"` termina el proceso
y no ofrece recuperación. Ver [panic and unwinding](./panic-and-unwinding.md).

## Validación F3/F12

Pruebas de ida/vuelta repetida, alineación, registros preservados y de control
FP, integridad de stack, migración, panic y retorno; inspección/revisión de cada
bloque Assembly y benchmark reproducible. Los tests de ABI/overflow fatal deben
aislarse en procesos donde un fallo no corrompa el test runner. Una prueba
unitaria de la API no sustituye validación de ABI.
