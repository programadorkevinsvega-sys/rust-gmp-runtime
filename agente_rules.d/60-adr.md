# Registros de decisiones arquitectónicas

- R-601 Crear un ADR en `docs/decisions/NNN-titulo.md` antes de implementar
  cualquier decisión que elija una estructura de datos o algoritmo, defina
  ownership o sincronización, introduzca `unsafe`/FFI/Assembly, cambie una API
  pública o una invariante, o descarte alternativas.
- R-602 Formato: Estado | Contexto | Decisión | Alternativas (con su coste) |
  Consecuencias | Riesgos | Cómo se valida.
- R-603 El PR referencia el ADR. Si un ADR queda obsoleto, marcarlo como
  reemplazado y crear uno nuevo; nunca reescribir la historia.
- R-604 Si un PR no necesita ADR, declarar el motivo en la plantilla y pedir al
  revisor que lo confirme.
