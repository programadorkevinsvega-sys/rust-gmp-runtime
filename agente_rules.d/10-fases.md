# Reglas de fases

- R-101 Una fase se cierra con todos sus entregables, el informe
  `docs/phases/phase-N-report.md` según el formato `PHASE REPORT` de
  `SPEC.md`, `scripts/check.sh` en verde y aprobación humana explícita. La
  aprobación puede darse escribiendo la línea
  `APROBADO FASE N: <nombre>, <fecha>` en el informe o confirmando en la
  conversación la fase concreta ya revisada.
- R-102 El agente nunca aprueba ni infiere aprobación. Si la persona ya aprobó
  explícitamente la fase en la conversación, el agente puede registrar en el
  informe la aprobación recibida, indicando fecha y que provino de esa
  conversación; no debe exigir que se repita manualmente.
- R-103 No crear archivos, tipos ni código de la fase N+1 mientras no exista la
  aprobación de la fase N. Si la fase N depende de algo de N+1, reportarlo.
- R-104 Tras aprobación y commit de cierre, una persona etiqueta el commit
  `phase-N-complete`.
- R-105 No presentar como completado un componente con código parcial.
