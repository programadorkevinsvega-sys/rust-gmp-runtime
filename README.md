# Rust GMP Runtime

**Runtime de concurrencia M:N stackful en Rust, basado en el modelo G-M-P.**

El objetivo es ejecutar muchas rutinas stackful con stacks propios sobre un número
limitado de threads del sistema operativo. El runtime implementará su propio
scheduler y gestión de ejecución: no es un wrapper, un thread pool convencional
ni un executor `async`.

![Rust 1.85](https://img.shields.io/badge/Rust-1.85%20%7C%20Edition%202024-orange?logo=rust)
![Arquitectura](https://img.shields.io/badge/arquitectura-G--M--P-blue)
![Ejecución](https://img.shields.io/badge/ejecuci%C3%B3n-M%3AN%20stackful-purple)
![Dependencias](https://img.shields.io/badge/crates%20externos-0-brightgreen)
![Estado](https://img.shields.io/badge/estado-F4.3%20en%20implementaci%C3%B3n-yellow)

## ¿Qué es?

`rust-gmp-runtime` es un proyecto de sistemas en Rust para diseñar e
implementar un runtime propio de rutinas stackful. Una rutina podrá suspenderse
y reanudarse conservando su stack, mientras el scheduler multiplexa esas tareas
sobre workers del sistema operativo.

F1-F4.2 están cerradas e integradas en `main`; F4.3 se implementa en la rama
`phase-4/minimal-execution-4-3`, en revisión en el
[PR #9](https://github.com/programadorkevinsvega-sys/rust-gmp-runtime/pull/9).
En Linux x86_64, un worker puede despachar G one-shot en un stack propio,
capturar su panic y devolver el resultado al llamador. Aún no hay
yield/reanudación ni scheduler M:N completo; los límites de esta fase están en
[PHASES.md](./PHASES.md).

## Características objetivo

- Scheduler M:N con modelo G-M-P y workers limitados.
- Rutinas síncronas stackful y cambio de contexto propio.
- Colas locales y globales, balanceo y work stealing.
- Suspensión y reanudación mediante primitivas cooperativas del runtime.
- Canales, sincronización y temporizadores.
- Netpoller y abstracción de plataforma.
- Gestión de memoria y métricas para evaluar consumo y escalabilidad.
- Cero crates externos; uso de Rust, su biblioteca estándar y APIs nativas
  justificadas.

## Arquitectura G-M-P

- **G — Rutina:** tarea ejecutable con estado, contexto y stack de dirección
  estable.
- **M — Máquina:** thread del sistema operativo que ejecuta rutinas y conserva
  su propio TLS. Los workers se asocian a un P y ejecutan dispatch one-shot.
- **P — Procesador:** capacidad lógica de ejecución que coordina la cola local y
  permite a una M ejecutar trabajo.

El scheduler coordina las asociaciones entre M y P, las colas y las transiciones
de estado de G. La arquitectura debe asegurar que una sola M ejecute cada
rutina a la vez y que recursos como el stack no se liberen mientras sigan en uso.

## Stacks

Cada rutina tiene un stack independiente de dirección estable. En Linux x86_64
el `StackManager` permite elegir dos motores de producción: Fixed, que sigue
siendo el default, y Go-inspired, seleccionable mediante `StackConfig`. Ambos
usan el mismo lease RAII desde `Routine`. Go-inspired reutiliza slots y puede
retener mappings; `trim` permite recuperar spans ociosos. Su selección no
implica que sea más escalable ni cambia el default. Las métricas y límites se
describen en [gestión de stacks](./docs/stack-management.md) y [ADR 016](./docs/decisions/016-production-stack-engines.md).

El diseño distingue memoria virtual reservada, páginas comprometidas y memoria
residente (RSS). La viabilidad de grandes cantidades de rutinas deberá
demostrarse con mediciones; no se promete que un millón de stacks sea viable.
En Linux, el Stack Manager consulta el límite real de VMA y los mappings del
proceso, y aplica por defecto preflight con margen configurable. Son snapshots
y estimaciones preventivas, no garantías de que el kernel acepte reservas
posteriores; si procfs no está disponible, el preflight falla explícitamente.

## Estado y plataforma

El único cambio de contexto implementado es one-shot para Linux x86_64/System
V AMD64; sus pruebas ABI dirigidas y la línea base inicial están en F3.3, aún
en revisión. No se afirma compatibilidad con AArch64, macOS o Windows: son
objetivos futuros sujetos a implementación y validación independientes.

El árbol conserva archivos fuente con solo comentarios para mantener la
estructura planificada. No son módulos compilados ni funcionalidades
implementadas; el estado real lo determinan los módulos declarados en `src/lib.rs`
y las pruebas.

## Documentación

- [STATUS.md](./STATUS.md): estado real y resumido del proyecto.
- [SPEC.md](./SPEC.md): especificación técnica y fuente de verdad arquitectónica.
- [PHASES.md](./PHASES.md): roadmap, entregables, dependencias y criterios de
  aceptación por fase.
- [docs/](./docs/): documentos técnicos, decisiones y reportes de fase.
- [Informe de cierre F2](./docs/phases/phase-2-report.md): implementación,
  verificaciones, aprobación y limitaciones documentadas.

El trabajo de implementación debe limitarse a la fase solicitada en
`PHASES.md`; no se deben adelantar fases futuras.

## Desarrollo

- Rust 1.85 o posterior compatible con `edition = "2024"`.
- No añadir dependencias externas.
- Mantener cada archivo de código en un máximo de 200 líneas físicas.
- Assembly y FFI únicamente en fronteras justificadas, documentadas y probadas.
- Ejecutar `./scripts/check.sh` desde este directorio para las verificaciones
  del proyecto.

## Licencia

Este proyecto está licenciado bajo cualquiera de las siguientes licencias, a tu elección:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

### Contribuciones

Salvo que se indique expresamente lo contrario, cualquier contribución enviada intencionadamente para su inclusión en este proyecto, según se define en la licencia Apache-2.0, tendrá doble licencia como se indica arriba, sin términos o condiciones adicionales.
