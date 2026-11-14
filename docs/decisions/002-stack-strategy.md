# ADR 002: Estrategia de stacks

- Estado: Aceptado: stacks fijos y acotados, sin crecimiento ni reuso inicial.
  Layout de reservas y tamaño exacto son gates de F2/F10 antes de estabilizar
  defaults.
- Fecha: 2026-11-14.

## Problema

Las referencias y punteros pueden depender de las direcciones de stack; cada G stackful requiere una región propia con límites y liberación segura.

## Alternativas y decisión

Alternativas: stack de tamaño fijo; reserva virtual y compromiso de páginas bajo
demanda; crecimiento por page-fault/handler; stacks copiables; pools de slots.

La decisión inicial es capacidad fija por G, direcciones estables y guard page
inaccesible, sin stack copying/relocation, growth ni pool. Un overflow no se
recupera: el proceso falla de manera explícita antes de permitir corrupción.
El tamaño no se fija con números especulativos: F2 medirá uso real (incluidos
formateo, recursión y locals grandes), y F10 revisará la capacidad/performance.

La reserva por stack es el baseline de corrección para probar guardia y
ownership. Antes de declarar soporte de gran escala, comparar con slabs en
syscalls, VMA, fragmentación, RSS y límites. No presumir que un slab evita VMA
por `mprotect` en cada slot. No cambiar `vm.max_map_count`.

## Costes de CPU y memoria

Stack fijo simplifica correctitud pero puede reservar espacio virtual no usado y
limitar profundidad. Guard/fault handling añade coste y riesgo; pools retienen
memoria y añaden limpieza/sincronización. Medir reserva, páginas tocadas/RSS,
syscalls, VMA, creación, liberación y retención por separado.

## Riesgos, límites y validación

Los valores 8 KiB inicial y 256 KiB reservado solo son candidatos de benchmark.
Pruebas F2/F12 deben aislar el overflow fatal, probar guardia, alineación,
depth/locals, liberación y errores de reserva. Pruebas F10 deben medir 1k/10k/
100k/500k/1M cuando recursos lo permitan. No declarar crecimiento ni
escalabilidad no medida.
