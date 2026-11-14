# Estado del proyecto

**Actualizado:** 2026-11-14
**Estado general:** F1 y F2 cerradas; F3.1 y F3.2 integradas; F3.3 en revisión
en el [PR #6](https://github.com/programadorkevinsvega-sys/rust-gmp-runtime/pull/6);
F4.1 en el [PR #7](https://github.com/programadorkevinsvega-sys/rust-gmp-runtime/pull/7)
y F4.2 implementada en una rama apilada sobre este.

## Fase actual

- **F1 — Arquitectura, fundamentos y decisiones:** cerrada.
- **F2 — Rutinas y gestión de stacks:** cerrada y fusionada en `main`.
- **F3.1 — Context switching x86_64:** fusionada en `main` mediante el
  [PR #4](https://github.com/programadorkevinsvega-sys/rust-gmp-runtime/pull/4).
- **F3.2 — Panic y unwinding:** fusionada en `main` mediante el
  [PR #5](https://github.com/programadorkevinsvega-sys/rust-gmp-runtime/pull/5).
- **F3.3 — Verificación y línea base:** implementada en la rama
  `phase-3/context-switching-3-3`; el PR #6 está abierto y sus checks de CI
  pasaron. No está fusionada todavía.
- **F4.1 — Processor:** implementada en `phase-4/processor-4-1`, apilada sobre
  F3.3; el [PR #7](https://github.com/programadorkevinsvega-sys/rust-gmp-runtime/pull/7)
  está abierto y apilado sobre el PR #6. Incluye pool configurable, cola local
  de rutinas publicadas y binding exclusivo ligado al thread; el ciclo de
  workers corresponde a F4.2 y el dispatch sigue pendiente.
- **F4.2 — Machine y worker:** implementada en
  `phase-4/machine-worker-4-2`, apilada sobre el PR #7. El manager crea un
  worker por Processor, registra identidad M en TLS, hace rollback de startup
  fallido y detiene/une los threads. Todavía no hay ejecución de G ni pinning;
  ambas condiciones quedan documentadas conforme a F1/F4.3.

## Estado de implementación

- En Linux x86_64, `Routine::run_once` ejecuta una entrada una sola vez en un
  stack alternativo, retorna al llamador y captura panic bajo unwind.
- No existe todavía yield/reanudación ni dispatch G-M-P. Los workers F4.2 se
  asocian con P, esperan y se detienen; todavía no ejecutan G. No se debe
  describir el proyecto como runtime M:N funcional.
- La implementación de cambio de contexto es específica de Linux x86_64/System
  V AMD64. No se declara soporte para otras plataformas.
- El repositorio conserva 76 archivos `.rs` de scaffold que contienen solo
  comentarios de módulo, para mantener el árbol planificado. No están
  declarados/compilados como módulos ni implementan las funcionalidades que
  nombran. No hay archivos vacíos de cero bytes ni directorios vacíos.
- No hay dependencias externas.

## Verificación más reciente

- `RUSTUP_TOOLCHAIN=1.85.0 ./scripts/check.sh`: pasó tras F4.2; 57 pruebas
  unitarias aprobadas y 2 ignoradas. Las suites de integración también pasaron.
- Los dos checks de CI del PR #6 pasaron.
- `cargo bench --bench context_switching`: diez muestras Linux x86_64; mediana
  `run_once` de 1.210 ns por rutina. Incluye trampoline, `catch_unwind`,
  lifecycle y primer toque del stack; no es una medición de scheduler ni de un
  cambio aislado. Datos y entorno en
  [la línea base F3.3](./docs/benchmarks/results/2026-11-14-f3-3-2978272.md).
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
