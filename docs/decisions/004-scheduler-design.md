# ADR 004: Diseño inicial del scheduler

- Estado: Aceptado como baseline de corrección para F5; fairness y parámetros
  quedan medidos en esa fase.
- Fecha: 2026-11-14.

## Problema

Las colas locales/globales y Work Stealing deben mantener ownership/estados correctos bajo concurrencia y parking eficiente.

## Alternativas y decisión

Colas lock-free desde el inicio (mayor riesgo de ABA, reclamación y carreras);
colas convencionales de `std` con optimización posterior guiada por medidas.

Usar inicialmente Mutex/condiciones de `std` para colas local, global e
inyección, con critical sections acotadas. Prohibido cruzar yield, suspensión,
context switch o I/O con lock tomado. Work stealing solo consume items
despublicados bajo transición exclusiva; consulta primero trabajo global y roba
lote acotado solo al quedar ocioso. Parking debe coordinar publicación y wake-up
para evitar lost wake-up y busy waiting. No se promete FIFO/fairness sin prueba.

Mutex puede añadir contención y bloquear M durante una sección; facilita
auditoría de ownership y línea base. Las colas/sincronizadores requieren
metadatos por P y retienen G; medir latency, throughput, contención y memoria.

## Riesgos, optimización futura y validación

Chase-Lev u otra cola lock-free requiere ADR nuevo y revisar ABA, memoria,
reclamación, ordenamientos y carreras; no es parte de F1/F5. F5 debe probar
publicación, stealing, parking/wake-up, shutdown, no doble ejecución y comparar
secuencias con modelo; stress y mediciones no demuestran ausencia de errores.
