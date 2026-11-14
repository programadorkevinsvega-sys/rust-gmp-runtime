# Seguridad

La implementación futura manejará stacks alternativos, Assembly, punteros, FFI
y sincronización. Priorizar correctitud y ownership sobre rendimiento. Los
directorios y stubs no prueban seguridad.

Encapsular `unsafe`; documentar necesidad, invariantes, aliasing, ownership, vida útil y alineación. Revisar data races, use-after-free, double-free, punteros colgantes, corrupción de stack, unwind, TLS, deadlocks, cancelación y liberación prematura.

No ejecutar un unwind fuera del límite controlado de G. No asumir que una rutina conserva TLS al migrar. No liberar handles o stacks mientras el SO/netpoller tenga operaciones pendientes. Ver [security invariants](./security-invariants.md), [panic and unwinding](./panic-and-unwinding.md) y [TLS/thread affinity](./tls-and-thread-affinity.md).

El runtime no será un sandbox: closures se ejecutan con los privilegios del
proceso y pueden bloquearlo o terminarlo. Ver [invariantes de seguridad](./security-invariants.md)
para los límites del modelo de amenazas.

**Estado:** requisitos de diseño, no informe de auditoría de una implementación.
