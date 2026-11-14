# ADR 003: Context switching y boundary de Assembly

- Estado: Contrato aceptado. F3.1 implementa el cambio one-shot y F3.2 captura
  panic en Linux x86_64/System V AMD64. La verificación ABI completa sigue
  pendiente de F3.3/F12.
- Fecha: 2026-11-14.

## Problema

Rust no expone una API estándar de cambio a stacks independientes. Se necesita conservar el estado de CPU y retornar al scheduler de forma controlada.

## Alternativas y decisión

Crate externo de context switching (prohibido); green threads async (cambia el
objetivo); context switch propio específico por ABI (complejidad de Assembly,
`unsafe` y unwind).

Implementar el cambio propio solo por backend de arquitectura detrás de una
interfaz privada; una API de rutina puede solicitar la ejecución one-shot, pero
no expone registros ni Assembly. Para Linux x86_64 bajo System V AMD64, preservar `rsp`, la
continuación, `rbx`, `rbp`, `r12`–`r15`, estado de control x87 y control MXCSR;
garantizar alineación de stack, red zone y DF según ABI. Un trampoline por G
controla la entrada y el retorno. No conservar TLS en el contexto. Ver
[`context-switching.md`](../context-switching.md).

Fuente primaria de revisión: [x86-64 psABI](https://gitlab.com/x86-psABIs/x86-64-ABI);
no se asume que documentar registros sustituya la revisión de la versión exacta
y el output del compilador.

## Costes de CPU/memoria y riesgos

Assembly/`unsafe`, unwinder, alineación, registros y stack requieren revisión
profunda. Cada G retiene el contexto además del stack; el espacio exacto se
medirá. El coste por switch se mide frente a ejecución directa y threads
nativos; no se presupone mejora.

## Panic y validación

F3.2 ejecuta una entrada `FnOnce + UnwindSafe` dentro de `catch_unwind` en el
stack de G y devuelve el payload por el resultado de `run_once`; el unwind no
cruza el frame `extern "C"` ni Assembly. Los locales se destruyen durante el
unwind, la rutina termina en `Panicked` y el llamador posee el payload. Doble
panic aborta; `panic = "abort"` no es recuperable. La API permanece experimental
hasta que F3.3/F12 validen registros/FP control, alineación, ida-vuelta,
migración, panic, destructores y scheduler. Cada arquitectura nueva requiere
ADR/backend y validación propios.
