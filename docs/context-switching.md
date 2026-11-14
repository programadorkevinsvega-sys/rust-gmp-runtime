# Context Switching

## Alcance

El cambio de contexto stackful es propio y específico de arquitectura. La
primera implementación prevista es Linux x86_64; no existe todavía una
implementación funcional que este documento certifique. En F2, cada rutina
almacena el límite superior de su stack y un slot vacío para el stack pointer
guardado. Ese límite no es un stack pointer inicial ABI-validado ni
ejecutable; F3 deberá preparar el trampoline y completar el estado CPU.

## Contrato acordado para Linux x86_64

El primer backend se limita a Linux x86_64 bajo System V AMD64 ABI. El cambio
será una operación privada entre contextos del runtime invocada como frontera
de función; no es una API pública ni una llamada para aplicaciones. El contexto
debe conservar `rsp`, el retorno/continuación y los registros callee-saved
`rbx`, `rbp`, `r12`–`r15`. El estado inicial debe cumplir la alineación de stack
de System V (16 bytes antes de `call`, 8 bytes al entrar a una función) y dejar
128 bytes de red zone disponibles debajo de `rsp`. El Direction Flag debe estar
limpio al entrar/salir de la frontera. El estado de control de x87 y los bits de
control de MXCSR que la ABI exige preservar también deben conservarse por G; los
registros vectoriales caller-saved no se consideran estado persistente entre
llamadas ordinarias.

El estado inicial entra por un trampoline controlado, nunca por una dirección
proporcionada por el usuario como retorno. Las transferencias ocurren solo con
G en estado `Running` y el token exclusivo en poder de M; al regresar, el
scheduler actualiza el estado antes de publicar o liberar recursos. El contexto
y stack son estables mientras G esté viva. El contexto no guarda ni restaura TLS:
la política de migración de G es independiente del estado TLS de M.

Fuentes de referencia: [System V AMD64 ABI (proyecto x86-psABIs)](https://gitlab.com/x86-psABIs/x86-64-ABI)
y [Intel 64 and IA-32 Architectures Software Developer Manuals](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).
Antes de escribir Assembly, cotejar la versión de ABI elegida, documentar
clobbers y revisar el compilador/metadata de unwinding. No asumir que x86_64 y
AArch64 comparten registros o ABI. Compilar no demuestra seguridad.

## Panic

El cuerpo de G debe ejecutarse bajo un límite controlado de `catch_unwind` cuando se habilite unwind. El panic se captura en el stack de G y se registra antes del retorno normal al scheduler; no se permite unwind a través de la frontera Assembly. Debe validarse que el unwinder funciona en el stack alternativo y que destructores/resultados se manejan. `panic = "abort"` termina el proceso y no ofrece recuperación. Ver [panic and unwinding](./panic-and-unwinding.md).

## Validación F3/F12

Pruebas de ida/vuelta repetida, alineación, registros preservados y de control
FP, integridad de stack, migración, panic y retorno; inspección/revisión de cada
bloque Assembly y benchmark reproducible. Los tests de ABI/overflow fatal deben
aislarse en procesos donde un fallo no corrompa el test runner. Una prueba
unitaria de la API no sustituye validación de ABI.
