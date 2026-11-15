# Estado del proyecto

**Estado general:** F1-F4.2 cerradas e integradas en `main`; F4.3 implementada
y verificada en la rama `phase-4/minimal-execution-4-3`, pendiente de revisión.

## Fase actual

- **F1-F4.2:** cerradas e integradas en `main`. F4.1 aporta Processor pool,
  colas locales y binding exclusivo; F4.2 añade un worker por P, registro M en
  TLS, rollback de startup y shutdown/join.
- **F4.3 — Ciclo mínimo de ejecución:** implementada en esta rama. Integra
  dispatch one-shot de G, context switching y un resultado observable mediante
  join. Incluye pruebas de concurrencia acotada, panic, rechazo posterior a
  shutdown y drenaje de trabajos aceptados. El gate completo pasó; pendiente de
  revisión en el [PR #9](https://github.com/programadorkevinsvega-sys/rust-gmp-runtime/pull/9).
  La estabilización valida 128/1k/10k G con 1/2/4 M y conserva una nueva línea
  base de 30 muestras y costes por componente en
  [el informe F4.3](./docs/benchmarks/results/2026-11-14-f4-3-stabilization-a32c69a.md).

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

- `RUSTUP_TOOLCHAIN=1.85.0 ./scripts/check.sh`: 62 pruebas aprobadas, 2
  pruebas manuales ignoradas; suites de integración y benchmarks registrados
  completan sin error.
- Las pruebas concurrentes ejecutan exactamente una vez 128, 1.000 y 10.000 G
  con 1, 2 y 4 M; cada entrada observa `Running`, y después del join cada G es
  terminal y cada P queda desasociado.
- `cargo bench --bench context_switching`: línea base F3.3 con diez muestras
  Linux x86_64; mediana
  `run_once` de 1.210 ns por rutina. Incluye trampoline, `catch_unwind`,
  lifecycle y primer toque del stack; no es una medición de scheduler ni de un
  cambio aislado. Datos y entorno en
  [la línea base F3.3](./docs/benchmarks/results/2026-11-14-f3-3-2978272.md).
- `cargo bench --bench machine_dispatch`: diez muestras Linux x86_64; throughput
  mediano de 194.715, 184.537 y 124.379 rutinas/s con 1, 2 y 4 workers,
  respectivamente. La carga corta no demuestra escalabilidad; entorno y
  resultados brutos en
  [línea base F4.3](./docs/benchmarks/results/2026-11-14-f4-3-bcb3a89.md).
- `RUSTUP_TOOLCHAIN=1.85.0 cargo bench --bench machine_dispatch_stabilized`:
  30 muestras, warm-up y orden rotativo. Mediana end-to-end no-op para 1/2/4 M:
  675.917/559.990/811.167 ns por batch de 128 G. El microbenchmark aislado
  `run_once` midió 183.293 µs por batch de 128; resultado completo y salida
  bruta en [el informe de estabilización](./docs/benchmarks/results/2026-11-14-f4-3-stabilization-a32c69a.md).
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
