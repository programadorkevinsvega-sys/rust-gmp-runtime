# Benchmarks reproducibles

- R-501 Cada implementación con impacto de rendimiento añade un benchmark en
  `benches/` y, cuando sea útil, una línea base equivalente con threads nativos
  de Rust. No añadir frameworks ni dependencias de benchmark.
- R-502 Benchmarks con `harness = false` y `std::time::Instant`; el
  harness de `cargo bench` es inestable.
- R-503 Condiciones comparables: mismo trabajo y límites de concurrencia,
  calentamiento y un mínimo de 10 ejecuciones; reportar mediana, p99 y
  dispersión.
- R-504 Registrar commit, `rustc -V`, CPU, kernel, governor y
  comando exacto. Guardar resultados en
  `docs/benchmarks/results/<fecha>-<commit>.md`.
- R-505 Las pruebas diferenciales comparan las implementaciones con un modelo
  de referencia determinista derivado de la semántica documentada (cierre de
  canales, selección, orden de despertar). Registrar las diferencias y su
  motivo en las pruebas o documentación propietaria del componente.
- R-506 Nunca afirmar mejoras de rendimiento sin datos reproducibles; explicar
  las diferencias semánticas y ambientales antes de comparar.
