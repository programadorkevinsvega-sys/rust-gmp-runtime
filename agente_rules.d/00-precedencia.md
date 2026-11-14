# Precedencia de instrucciones

1. Las instrucciones permanentes del proyecto y las del usuario se respetan.
   Si el usuario contradice una regla obligatoria de este proyecto, detenerse y
   pedir confirmación explícita citando el identificador de la regla.
2. `SPEC.md` es el contrato técnico principal. Las reglas de `AGENTE.md` lo
   endurecen, pero no reemplazan su arquitectura ni autorizan adelantar fases.
3. Entre archivos de `agente_rules.d/`, prevalece el de numeración más baja
   cuando haya conflicto.
4. La fase solicitada en `PHASES.md` limita el alcance de cada implementación.
5. Si un conflicto no puede resolverse con estas reglas, no improvisar:
   documentar el conflicto y pedir una decisión humana.
