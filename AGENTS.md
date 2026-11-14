# AGENTS.md — Reglas del proyecto rust-gmp-runtime

Estas reglas son obligatorias. Si una instrucción del usuario las contradice,
detente y pide confirmación explícita citando el ID de la regla. El detalle de
cada regla está en `agente_rules.d/`. Ante conflicto, prevalece el archivo de
numeración más baja. La especificación maestra manda sobre estas reglas, salvo
en lo que estas endurecen.

## Reglas innegociables

- R-001 Nunca avances a la fase N+1 sin cierre formal de la fase N
  (`agente_rules.d/10-fases.md`).
- R-002 Cada implementación = una rama, un PR y commits atómicos
  (`agente_rules.d/20-git-pr.md`).
- R-003 Prohibido TODO, FIXME, XXX, HACK, `todo!()` y `unimplemented!()`.
- R-004 Prohibido descartar valores o errores: sin `let _ =`, sin `_` como
  descarte, sin `.ok()` sobre `Result`, sin `Err(_)`, sin `map_err(|_| ...)`.
- R-005 Todo camino que pueda fallar devuelve un error tipado, claro y con
  contexto.
- R-006 Toda implementación incluye tests y fuzzing/pruebas basadas en modelo
  (`agente_rules.d/40-pruebas-fuzzing.md`).
- R-007 Toda implementación con impacto de rendimiento incluye benchmarks
  reproducibles contra una línea base de threads nativos de Rust
  (`agente_rules.d/50-benchmarks.md`).
- R-008 Toda decisión de diseño tiene un ADR antes de implementarse
  (`agente_rules.d/60-adr.md`).
- R-009 Todo `unsafe` está encapsulado, documentado y probado
  (`agente_rules.d/70-unsafe-seguridad.md`).
- R-010 Nunca declares algo como terminado si no lo has ejecutado y verificado.
  Reporta resultados reales; nunca inventes cifras ni resultados.
- R-011 Ningún archivo de código puede superar 200 líneas físicas; dividirlo
  en módulos pequeños y validar el límite con `scripts/check-forbidden.sh`
  (`agente_rules.d/80-limite-archivos.md`).

## Definición de «hecho»

Un cambio solo está hecho si `scripts/check.sh` termina con código 0 y el PR
cumple la plantilla completa. Si no puedes cumplir una regla, no la saltes:
explica qué lo bloquea, qué alternativas hay, su coste y qué recomiendas.

## Integración Git

Este directorio es la raíz de su propio repositorio. Activar el hook local con
`git config core.hooksPath .githooks`. GitHub carga los workflows y la plantilla
de PR desde `.github/` en esta raíz.
