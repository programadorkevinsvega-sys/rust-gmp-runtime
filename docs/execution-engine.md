# Motor de ejecución

El motor conecta scheduler, contexto y stack de G. La secuencia esperada es: el scheduler marca G como Running y posee el derecho de ejecución; el backend de arquitectura instala/restaura su contexto; G cede, espera o termina por un trampoline conocido; el backend retorna al scheduler y este actualiza el estado antes de publicar/reutilizar recursos.

La transferencia de ejecución no es una llamada Rust ordinaria. Sus contratos
de stack, ABI, registros y `unsafe` están definidos para la primera plataforma
en [context-switching.md](./context-switching.md); el ciclo de vida del stack en
[stack-management.md](./stack-management.md). La implementación y sus pruebas
siguen pendientes.

No asumir que cualquier operación bloqueante de Rust cede el Processor. Las primitivas del runtime y los adaptadores explícitos deben administrar suspensión; ver [io-strategy.md](./io-strategy.md).

**Estado:** arquitectura objetivo; sin motor funcional verificado.
