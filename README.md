# Rust GMP Runtime

Runtime de concurrencia M:N stackful en Rust, inspirado en el modelo G-M-P de Go. El objetivo es implementar goroutines y scheduler propios, sin crates externos; no es un wrapper, un thread pool convencional ni un executor async.

## Estado

Proyecto en fase de diseño. El scaffold y los documentos describen objetivos; no representan una implementación funcional. La primera plataforma prevista es Linux x86_64.

## Documentación normativa

- [SPEC.md](./SPEC.md): contrato técnico y fuente de verdad arquitectónica.
- [PHASES.md](./PHASES.md): fases, dependencias, pruebas y criterios de avance.
- [docs/](./docs/): decisiones y documentos técnicos específicos.

En cada tarea de desarrollo, usar la especificación, únicamente la fase solicitada de `PHASES.md`, el código existente y las instrucciones concretas. No adelantar fases posteriores.

## Restricciones

- Rust 1.85, edición 2024; comprobar MSRV en CI y no usar APIs inestables.
- `[dependencies]` y `[dev-dependencies]` deben permanecer vacías.
- Assembly/FFI solo en fronteras justificadas, documentadas y probadas.
- No versionar `Cargo.lock` para esta biblioteca sin dependencias; reconsiderarlo si aparece un binario/workspace de pruebas con una razón concreta.

Las limitaciones de TLS, panics, stacks, I/O bloqueante y compatibilidad por plataforma están descritas en `SPEC.md`.

## Licencia

Este proyecto está licenciado bajo cualquiera de las siguientes licencias, a tu elección:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

### Contribuciones

Salvo que se indique expresamente lo contrario, cualquier contribución enviada intencionadamente para su inclusión en este proyecto, según se define en la licencia Apache-2.0, tendrá doble licencia como se indica arriba, sin términos o condiciones adicionales.
# rust-gmp-runtime
