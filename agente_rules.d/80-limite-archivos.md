# Límite de tamaño de archivos

- R-801 Ningún archivo de código del proyecto puede superar 200 líneas
  físicas. El límite incluye líneas en blanco y comentarios; no aplica a
  documentación ni a archivos generados/dependencias.
- R-802 Al alcanzar el límite, separar responsabilidades en módulos pequeños
  con nombres descriptivos. No reducir el conteo mediante líneas largas,
  minificación, eliminación de formato o traslado artificial de código.
- R-803 `scripts/check-forbidden.sh` comprueba el límite para archivos `.rs` y
  `.sh` del repositorio. La misma regla aplica a otros lenguajes de código,
  aunque el verificador aún no los cubra.
