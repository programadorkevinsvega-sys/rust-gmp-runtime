# Panics y unwinding

## Decisión de contrato

Cada rutina comienza en un trampoline controlado. Con `panic = "unwind"`, el
cuerpo se ejecuta dentro de `catch_unwind` en el stack propio de esa G. El panic
se captura en el stack de G y se convierte en resultado terminal antes del
retorno normal al scheduler; no se permite unwind a través del límite Assembly.
El hook de panic estándar puede ejecutarse antes de la captura y sus efectos
siguen siendo los del proceso.

`Routine::run_once` devuelve éxito, `RoutineRunError::Panicked(payload)` o un
fallo de runtime. El llamador es dueño del payload y puede inspeccionarlo o
descartarlo; su destructor se ejecuta en el stack del llamador. No se suprime ni
reemplaza el panic hook. La API permanece experimental hasta completar las
puertas de verificación ABI de F3.3/F12.

Las pruebas unitarias actuales demuestran la captura en stack alternativo y el
retorno al stack llamador en Linux x86_64; no certifican por sí solas el
unwinder/ABI para todos los usos. Con `panic = "abort"` el proceso termina sin
captura ni continuidad; la biblioteca no impondrá `panic = "abort"` a
dependientes. El ejemplo `examples/context_panic_abort.rs` sirve para validar
esa configuración en un proceso separado.

El unwinder destruye los frames y locales del cuerpo de G hasta el límite
`catch_unwind`. Tras capturar, el trampoline conserva el payload, marca G
`Panicked` y retorna al scheduler por el camino normal. Si un destructor
paniquea durante unwinding se produce un doble panic y el proceso aborta; no
convertir ese caso en recuperación. El destructor del payload devuelto queda
bajo responsabilidad del llamador. La política actual no cancela otras G
automáticamente por el panic de una G; aún no existe ownership de tareas
dependientes.

Stack, locks del scheduler y recursos pendientes no se liberan desde una ruta
que todavía puede ejecutarlos o referenciarlos. Toda operación nativa debe
completar o cancelarse antes de finalizar G. Los aborts de allocator/proceso son
terminales, no fallos recuperables de G.

## Pruebas F3/F12

- Panic en una G y verificación del resultado y retorno al scheduler.
- Destrucción de locales durante unwind y ownership/descarte explícito del payload.
- Continuidad de rutinas independientes; no hay cancelación implícita de tareas.
- Panic con stack alternativo bajo unwind, sin atravesar el límite Assembly.
- Configuración abort en proceso separado; nunca afirmar que `catch_unwind` lo captura.
- Doble panic en destructor durante unwind; debe abortar, no recuperarse.
- Shutdown concurrente con panic y garantía de que los locks quedan liberados.

**Estado:** captura y retorno one-shot probados en Linux x86_64; el ejemplo
`panic = "abort"` también se ejecutó en un proceso separado y terminó con
`SIGABRT` (código 134) en esta máquina. Validación ABI ampliada y pruebas de
estrés siguen siendo puertas antes de prometer estabilidad.
