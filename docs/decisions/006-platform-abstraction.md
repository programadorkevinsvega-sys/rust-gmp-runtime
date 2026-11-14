# ADR 006: Abstracción de plataforma

- Estado: Aceptado para el alcance inicial Linux x86_64; integración de cada
  API nativa requiere revisión/validación de su subfase.
- Fecha: 2026-11-14.

## Problema

Separar APIs/ABI de SO y arquitectura sin crear una abstracción que aplaste semánticas distintas ni stubs no funcionales.

## Alternativas y decisión

Generalizar todo bajo un modelo `epoll` (rechazado porque confunde readiness y
completion); interfaces internas que comparten identidad/ciclo de vida cuando
corresponde, con backends readiness/completion explícitos.

Linux x86_64 es el único primer target. Las otras plataformas requieren fases y
pruebas propias; nombres de archivos/`cfg` no constituyen soporte. Usar `std`
para threads y sincronización. Memoria protegida se aísla en una API privada
específica del target; `epoll` readiness se difiere a F9. No declarar una
abstracción portable que oculte diferencia de evento/resultado.

Más backends añaden dispatch/ramas y estructuras por operación pendiente.
Llamadas nativas tienen costes y errores distintos. No crear threads con syscall
cruda; no asumir que ABI de kernel equivale a libc. Medir CPU/memoria por backend.

## Validación

Documentar fuente/ABI/errores por llamada; compilar y probar cada
SO/arquitectura antes de declarar compatibilidad. F9 probará sockets/pipes y
documentará archivos regulares no multiplexables. F13 añadirá cada destino
solamente con backend de memoria/contexto e I/O y matriz de pruebas aprobados.
