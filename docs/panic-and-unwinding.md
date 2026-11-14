# Panics y unwinding

## Decisión de contrato

Cada rutina comienza en un trampoline controlado. Con `panic = "unwind"`, el
cuerpo se ejecuta dentro de `catch_unwind` en el stack propio de esa G. El panic
se captura en el stack de G y se convierte en resultado terminal antes del
retorno normal al scheduler; no se permite unwind a través del límite Assembly.
El hook de panic estándar puede ejecutarse antes de la captura y sus efectos
siguen siendo los del proceso.

El resultado conceptual de G es éxito, panic capturado o fallo del runtime. El
handle de unión observa el éxito o el error/payload; su ownership debe permitir
descartar el payload si no se une. No se suprime ni reemplaza el panic hook.
`catch_unwind` no demuestra que el unwinder soporte la pila alternativa, que la
ABI sea correcta ni que el contexto sea íntegro.

La ruta solo se puede prometer como API después de las pruebas F3/F12 del
unwinder en stack alternativo. Con `panic = "abort"` el proceso termina sin
captura ni continuidad; la biblioteca no impondrá `panic = "abort"` a
dependientes.

El unwinder destruye los frames del cuerpo de G hasta el límite `catch_unwind`.
Tras capturar, el trampoline publica el resultado, marca G terminal y retorna al
scheduler por el camino normal. Si un destructor paniquea durante unwinding
puede producir abort; no convertir ese caso en recuperación. La primera política
no cancela otras G automáticamente por el panic de una G.

Stack, locks del scheduler y recursos pendientes no se liberan desde una ruta
que todavía puede ejecutarlos o referenciarlos. Toda operación nativa debe
completar o cancelarse antes de finalizar G. Los aborts de allocator/proceso son
terminales, no fallos recuperables de G.

## Pruebas F3/F12

- Panic en una G y verificación del resultado y retorno al scheduler.
- Liberación/destrucción del estado y continuidad de otras G.
- Panic con stack alternativo bajo unwind, sin atravesar el límite Assembly.
- Configuración abort en proceso separado; nunca afirmar que `catch_unwind` lo captura.
- Payload conservado o descartado explícitamente; destructor del payload que paniquea.
- Shutdown concurrente con panic y garantía de que los locks quedan liberados.

**Estado:** contrato de diseño elegido; ABI y comportamiento real del unwinder
siguen siendo puertas de validación antes de prometer recuperación.
