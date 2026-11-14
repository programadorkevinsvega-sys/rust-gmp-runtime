# Estado del proyecto

**Estado general:** F1-F4.2 cerradas e integradas en `main`; F4.3 implementada
en la rama `phase-4/minimal-execution-4-3`, pendiente de verificación final y
revisión.

## Fase actual

- **F1-F4.2:** cerradas e integradas en `main`. F4.1 aporta Processor pool,
  colas locales y binding exclusivo; F4.2 añade un worker por P, registro M en
  TLS, rollback de startup y shutdown/join.
- **F4.3 — Ciclo mínimo de ejecución:** implementada en esta rama. Integra
  dispatch one-shot de G, context switching y un resultado observable mediante
  join. Incluye pruebas de concurrencia acotada, panic, rechazo posterior a
  shutdown y drenaje de trabajos aceptados. Pendiente de gate completo y PR.

## Estado de implementación

- En Linux x86_64, cada worker ejecuta una G una sola vez en su stack
  alternativo, retorna al stack del worker y entrega resultado o panic por un
  join handle. El permiso de G se adquiere antes de publicar M como `Running`.
- El dispatch round-robin usa workers existentes: 20 G pueden ejecutarse con
  dos M; el número de threads no crece con el número de tareas. No existe aún
  yield/reanudación, scheduler general, balanceo ni runtime M:N funcional.
- La implementación de cambio de contexto es específica de Linux x86_64/System
  V AMD64. No se declara soporte para otras plataformas.
- El repositorio conserva 76 archivos `.rs` de scaffold que contienen solo
  comentarios de módulo, para mantener el árbol planificado. No están
  declarados/compilados como módulos ni implementan las funcionalidades que
  nombran. No hay archivos vacíos de cero bytes ni directorios vacíos.
- No hay dependencias externas.

## Verificación más reciente

- `cargo test machine::tests`: 6 pruebas aprobadas después del cambio de
  dispatch. El gate completo de F4.3 está pendiente.
- `cargo bench --bench context_switching`: línea base F3.3 con diez muestras
  Linux x86_64; mediana
  `run_once` de 1.210 ns por rutina. Incluye trampoline, `catch_unwind`,
  lifecycle y primer toque del stack; no es una medición de scheduler ni de un
  cambio aislado. Datos y entorno en
  [la línea base F3.3](./docs/benchmarks/results/2026-11-14-f3-3-2978272.md).
- El benchmark `machine_dispatch` mide diez batches de 128 G con 1, 2 y 4
  workers. La línea base F4.3 se añadirá tras ejecutar el benchmark sobre el
  commit validado.
- El límite de 200 líneas aplica a archivos de código, no a Markdown. El
  verificador reporta cero archivos `.rs`/`.sh` por encima del límite y cero
  archivos/directorios vacíos.

## Documentos de referencia

- [SPEC.md](./SPEC.md): especificación técnica.
- [PHASES.md](./PHASES.md): roadmap y criterios por fase.
- [Informe de F1](./docs/phases/phase-1-report.md): decisiones, verificaciones y
  limitaciones de la fase.
- [Metodología de benchmarks](./docs/benchmarks/methodology.md): procedimientos
  y límites de interpretación.
