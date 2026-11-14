# Estado del proyecto

**Actualizado:** 2026-11-14
**Estado general:** diseño arquitectónico terminado; implementación del runtime
aún no iniciada.

## Fase actual

- **F1 — Arquitectura, fundamentos y decisiones:** aprobada por la persona
  usuaria y PR [#1](https://github.com/programadorkevinsvega-sys/rust-gmp-runtime/pull/1)
  integrado en `main`; cerrada administrativamente con la etiqueta
  `phase-1-complete`.
- **F2 — Rutinas y gestión de stacks:** no iniciada; requiere una solicitud
  específica para comenzar.

## Estado de implementación

- El código actual es scaffold; no existe todavía un runtime funcional.
- Se mantiene el objetivo M:N stackful con arquitectura G-M-P.
- No hay dependencias externas.
- La primera plataforma objetivo es Linux x86_64.

## Verificación más reciente

- `./scripts/check.sh`: pasó en la rama de F1 antes de crear el PR.
- Los archivos de test compilan, pero ejecutan **0 tests funcionales**.
- No hay benchmarks de rendimiento; se documentó la metodología para fases
  futuras.

## Documentos de referencia

- [SPEC.md](./SPEC.md): especificación técnica.
- [PHASES.md](./PHASES.md): roadmap y criterios por fase.
- [Informe de F1](./docs/phases/phase-1-report.md): decisiones, verificaciones y
  limitaciones de la fase.
