# ADR 005: Gestión de memoria

- Estado: Aceptado: allocator de Rust/RAII para objetos; sin allocator global
  propio ni reuso de stacks inicialmente.
- Fecha: 2026-11-14.

## Problema

Reducir memoria y asignaciones sin introducir liberación prematura, fragmentación o complejidad no justificada.

## Alternativas y decisión

Allocator global propio (rechazado sin evidencia); allocator estándar y
estructuras internas selectivas con RAII; GC (fuera del objetivo/arquitectura).

Usar ownership, borrowing y RAII; no introducir GC ni reemplazar allocator
global sin benchmark reproducible y caso técnico. No pool de stacks al inicio.
Evaluar slabs/arenas/pools para objetos solo cuando el perfil identifique un
patrón y existan límites de retención/limpieza. Slabs virtuales de stacks son
una decisión separada del allocator de objetos.

Pools pueden reducir alloc/free pero retener memoria y añadir sincronización y
limpieza. El allocator estándar puede tener overhead, pero evita complejidad
prematura. Medir CPU y memoria antes/después, incluyendo virtual, comprometida,
RSS, capacidad libre y fragmentación.

## Riesgos y validación

Confundir reservado, comprometido y residente; liberar stack mientras una
referencia/operación pendiente lo usa; reusar estado contaminado. F2/F10 miden
baseline, alloc/free, memoria, límites, fallos, presión, reuso/liberación y
concurrencia. No afirmar capacidad que no se haya probado.
