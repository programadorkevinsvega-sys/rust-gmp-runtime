# Reglas de Git y Pull Requests

- R-201 Una rama por implementación:
  `phase-N/<area>-<descripcion>`.
- R-202 Commits atómicos con Conventional Commits (`feat:`, `fix:`, `test:`,
  `docs:`, `bench:`, `adr:`).
- R-203 Un PR por implementación, con la plantilla completa. Mantener los PR
  pequeños y revisables.
- R-204 Cada commit compila y pasa las pruebas. No crear commits «wip».
- R-205 Prohibido `git push --force` a ramas compartidas, `--no-verify` y
  desactivar hooks o CI.
- R-206 El agente no hace merge a `main`; lo hace una persona tras la revisión.
