# Compatibilidad de plataformas

## Orden

1. **Linux x86_64:** único target inicial para la primera implementación.
2. Linux AArch64, después de ABI/context-switching propio.
3. macOS x86_64 y AArch64, con backend de memoria y `kqueue` propio.
4. Windows x86_64, con su estrategia de memoria/contexto e I/O completion IOCP.
5. Windows AArch64 solo tras evaluación de viabilidad.

La arquitectura y sus nombres de archivo no significan soporte. Exigir build
real y pruebas de stack, context switch, scheduler y backend I/O aplicable en
cada target antes de anunciar compatibilidad. Ningún destino distinto de Linux
x86_64 está implementado o validado hoy.

## Fronteras

Usar `std::thread`/`std::sync` cuando baste. Aislar APIs nativas para memoria
protegida y polling que no tengan alternativa suficiente. Verificar por separado
ABI libc, syscall kernel y convenciones de llamada de arquitectura; no inventar
bindings ni declarar que ABI libc y kernel son equivalentes.

El netpoller distingue readiness (`epoll`, `kqueue`) de completion (IOCP).
Compartir solo conceptos con semántica común; no aplanar operaciones/eventos
distintos.

## Estado

Las plataformas futuras son objetivos, no capacidades implementadas. Mantener
decisiones de portabilidad revisables sin crear stubs inútiles en el código.
