# Modelo de memoria

## Ownership y ciclo de vida

- Runtime controla admisión, workers y registro de tareas hasta completar el
  drenaje.
- G posee closure, estado de resultado, contexto y owner del stack mientras su
  ejecución está activa.
- Colas contienen referencias de G `Runnable`; sacar una entrada no basta por sí
  sola: un token/transición exclusiva debe autorizar `Running`.
- Waiters y operaciones nativas conservan las referencias/handles necesarios
  hasta completar o confirmar cancelación. No se libera G/stack por timeout
  local si el backend aún puede acceder a ellos.
- Join handles observan resultados y no poseen permiso para liberar stack
  activo.

Usar RAII y ownership de Rust; no introducir GC. Los detalles de representación
`Arc`, `Pin`, atomics y memory order se seleccionarán en cada fase tras definir
invariantes, no por anticipado.

## Memoria virtual y residente

Reportar por separado:

- Espacio virtual reservado para stacks.
- Páginas habilitadas/comprometidas según la interfaz elegida.
- Memoria residente observada (RSS), cuando pueda medirse.
- Metadatos, buffers y memoria de pools.

La demanda de páginas del SO no equivale a stack growth administrado por el runtime. Valores de 8 KiB iniciales y 256 KiB reservados son únicamente parámetros de benchmark, no defaults aprobados.

## Decisión inicial de memoria

Mantener direcciones estables y stack de capacidad fija, sin crecimiento ni
reutilización inicial. Usar el allocator estándar para metadatos/objetos; no
sustituir el allocator global. Para stacks, comparar en F2/F10 mapeos
individuales y slabs; la opción final debe reportar tanto syscalls como VMA y
medirse contra límites del entorno. `mprotect` puede dividir una reserva en
varias VMAs; un slab no garantiza eliminar presión de `vm.max_map_count`.

El pool solo se considerará tras resolver el ciclo de vida de G y probar
limpieza, protección, referencias pendientes, retención máxima y retorno al SO.

## Métricas obligatorias y riesgos

Medir stacks totales/activos/libres/reutilizados/liberados, slabs, capacidad,
mapeos/VMA, virtual reservado, páginas comprometidas/tocadas, RSS, fragmentación,
metadatos/bytes por G, asignaciones, latencia y fallos. Registrar las cargas de
1k, 10k, 100k, 500k y 1M solo hasta donde lo permitan los recursos; no modificar
límites globales. Revisar fragmentación, agotamiento, liberación prematura,
aliasing, concurrencia y presión de memoria. Ver [stack management](./stack-management.md)
y [security invariants](./security-invariants.md).

El Stack Manager expone por separado el snapshot real (`vma_limit`,
`vma_current`, `vma_remaining`, `vma_utilization_percent`), el número de
mappings RAII del runtime (`runtime_mapping_count`) y la cota interna
(`runtime_vma_estimate`). La última no se presenta como conteo exacto de VMAs
propias. El preflight predeterminado deja 1.024 VMA configurables y actualiza
procfs periódicamente; la estimación de operaciones internas evita que reservas
concurrentes del mismo manager consuman dos veces la misma capacidad observada.
La lectura del límite y del snapshot no garantiza reservas futuras.
