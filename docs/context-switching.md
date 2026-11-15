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
Los registros callee-saved se guardan en el stack mediante `push`/`pop`; el
objeto de contexto solo conserva el stack pointer y los controles FP (16 bytes).

El trampoline inicial recibe un frame interno en `r12` y llama a una función
Rust que ejecuta la entrada dentro de `catch_unwind` en el stack alternativo.
Solo después de capturar el resultado vuelve al frame `extern "C"` y al
Assembly; el unwind nunca cruza ese límite. `Routine::run_once` exige
`FnOnce + UnwindSafe` y adquiere primero el `ExecutionPermit` (estado
`Running`). El trampoline conserva resultado y permiso mientras vuelve al stack
del scheduler. Solo cuando el context switch ha retornado y el stack de G ya no
está en ejecución, el scheduler completa el permiso y marca `Dead/Completed` o
`Dead/Panicked`. En panic se destruyen los locales desenrollados y se conserva
el payload para el llamador. El llamador es dueño del payload y puede
inspeccionarlo o descartarlo. Una inconsistencia que impida una transición
segura posterior a la transferencia es fail-stop. La API aún no permite
pausar/reanudar G y sigue siendo experimental hasta completar F3.3/F12.

El estado inicial entra por un trampoline controlado, nunca por una dirección
proporcionada por el usuario como retorno. Esta implementación solo transfiere
con G en estado `Running` y el token exclusivo en poder del context switch; el
scheduler completa la transición a `Dead` después de volver al stack del worker.
Contexto y stack permanecen estables durante la ejecución. El contexto no guarda
ni restaura TLS;
la política de migración de G es independiente del estado TLS de M.

Fuentes: [System V AMD64 ABI (proyecto x86-psABIs)](https://gitlab.com/x86-psABIs/x86-64-ABI)
y [Intel 64 and IA-32 Architectures Software Developer Manuals](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).
El cambio de contexto de bajo nivel permanece encapsulado en
`src/context/arch/x86_64.rs`; no asumir que x86_64 y AArch64 comparten
registros o ABI.

## Panic

Con `panic = "unwind"`, el panic hook estándar corre y luego el panic se captura
en el stack de rutina. Los errores de preparación se devuelven como fallos del
runtime; los fallos internos inseguros después de cambiar de stack son
fail-stop. No se cancelan otras rutinas como consecuencia del panic; todavía no
hay ownership de tareas dependientes ni scheduler integrado. Si un destructor
paniquea durante unwinding, el doble panic aborta. El destructor del payload
que devuelve `run_once` corre bajo responsabilidad del llamador y puede
panicar allí.

Con `panic = "abort"` el proceso termina sin captura ni continuidad; la
biblioteca no impone ese modo a sus dependientes. El ejemplo
`examples/context_panic_abort.rs` permite comprobarlo con
`RUSTFLAGS="-C panic=abort" cargo run --example context_panic_abort`. Ver
[panic and unwinding](./panic-and-unwinding.md).

## Validación F3/F12

Las pruebas Linux x86_64 verifican captura de panic en stack alternativo,
retorno al llamador, estado terminal, destrucción de locales y ownership del
payload. Una prueba con `RUST_BACKTRACE=1` pasa con stack de 32 KiB; 8 KiB no
está validado y el probe actual terminó con `SIGSEGV`. F3.3 añade 32 ejecuciones
one-shot independientes, una llamada Rust
anidada en el stack alternativo, alineación en la entrada a una función C,
`rbx`/`rbp`/`r12`–`r15` y restauración de los modos de redondeo x87/MXCSR. No
prueba yield/reanudación del mismo contexto ni migración entre workers. El
benchmark `context_switching` registra una línea base de dispatch/retorno
one-shot y una llamada directa; no mide un scheduler ni un switch aislado.
Las rutas abort/fatal se aíslan en procesos donde un fallo no corrompa el test
runner. Una prueba unitaria no sustituye la revisión ABI completa de F12.
