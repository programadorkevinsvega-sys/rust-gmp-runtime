# Estado del proyecto

**Actualizado:** 2026-11-14
**Estado general:** F1 y F2 cerradas; F3.1 y F3.2 integradas; F3.3 en revisión
en el [PR #6](https://github.com/programadorkevinsvega-sys/rust-gmp-runtime/pull/6).

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

## Estado de implementación

- En Linux x86_64, `Routine::run_once` ejecuta una entrada una sola vez en un
  stack alternativo, retorna al llamador y captura panic bajo unwind.
- No existe todavía yield/reanudación, scheduler, workers ni integración G-M-P;
  no se debe describir el proyecto como runtime M:N funcional.
- La implementación de cambio de contexto es específica de Linux x86_64/System
  V AMD64. No se declara soporte para otras plataformas.
- El repositorio conserva 76 archivos `.rs` de scaffold que contienen solo
  comentarios de módulo, para mantener el árbol planificado. No están
  declarados/compilados como módulos ni implementan las funcionalidades que
  nombran. No hay archivos vacíos de cero bytes ni directorios vacíos.
- No hay dependencias externas.

## Verificación más reciente

- `RUSTUP_TOOLCHAIN=1.85.0 ./scripts/check.sh`: pasó; 47 tests aprobados y 2
  ignorados.
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
