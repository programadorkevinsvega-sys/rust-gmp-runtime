# Auditoría de concurrencia del allocator a alta cardinalidad

## Alcance y entorno

Se midió el allocator Fixed Stack actual en Linux x86_64 con Rust 1.85.0,
manteniendo 100.000 stacks vivos durante el ciclo de medición. El host informó
un límite de 1.048.576 VMA y 45 VMA iniciales. Los 100.000 stacks retenidos
correspondieron a 200.031 VMA observadas. El entorno de CPU y kernel es el
mismo que en la [auditoría VMA](./2026-11-14-vma-performance.md).

Se lanzaron 1, 2, 4 u 8 workers, con diez repeticiones por configuración; cada
worker realizó 5.000 ciclos de `allocate()` seguido por `Drop`. Se compararon
un snapshotter concurrente desactivado y activado. El workload desactiva el
preflight VMA en cada asignación para que el escenario sin snapshots no dispare
lecturas implícitas; por tanto, el modo activado mide snapshots solicitados
concurrentemente, no la reconciliación por rechazo preventivo.

La feature opt-in `allocator-benchmark` instrumenta el camino real. Cada worker
tiene sus propios contadores de espera, secciones críticas y syscalls; el
thread snapshotter se contabiliza aparte. Los percentiles de latencia incluyen
las operaciones de los diez lotes. La instrumentación añade overhead y estos
datos no deben interpretarse como rendimiento sin instrumentación.

## Throughput y latencia del ciclo completo

| Snapshots | Workers | Operaciones | Throughput mediano (ops/s) | p50 (ns) | p95 (ns) | p99 (ns) | Máximo (ns) |
|---|---:|---:|---:|---:|---:|---:|---:|
| Off | 1 | 50.000 | 289.460 | 2.579 | 5.735 | 8.057 | 203.023 |
| Off | 2 | 100.000 | 238.503 | 6.776 | 13.735 | 18.777 | 1.205.670 |
| Off | 4 | 200.000 | 202.387 | 19.210 | 34.492 | 58.473 | 3.473.841 |
| Off | 8 | 400.000 | 182.731 | 45.198 | 87.390 | 131.401 | 3.208.292 |
| On | 1 | 50.000 | 262.228 | 2.953 | 6.511 | 8.642 | 529.250 |
| On | 2 | 100.000 | 235.114 | 7.634 | 15.459 | 21.215 | 3.215.916 |
| On | 4 | 200.000 | 182.399 | 21.670 | 42.065 | 66.840 | 3.291.951 |
| On | 8 | 400.000 | 156.173 | 51.361 | 106.134 | 170.873 | 2.535.723 |

Con snapshots apagados, el throughput mediano cae 36,8 % de uno a ocho workers,
mientras p99 sube de 8,1 µs a 131,4 µs. Con snapshots activos, cae 40,4 % y
p99 llega a 170,9 µs. La carga no escala linealmente con workers.

## Espera, sección crítica y syscalls

Los siguientes valores son medianas de los promedios calculados por cada worker
y repetición: espera por adquisición, duración por tramo crítico y duración
por syscall, en nanosegundos. La tabla resume workers de cada configuración;
el log bruto conserva los valores de cada worker y muestra sus diferencias.
Cada ciclo normalmente cuenta dos adquisiciones/tramos críticos, y una llamada
de cada syscall.

| Snapshots | Workers | Espera Mutex/adquisición | Mutex mantenido/tramo | mmap | mprotect | munmap |
|---|---:|---:|---:|---:|---:|---:|
| Off | 1 | 21 | 1.538 | 972 | 827 | 1.090 |
| Off | 2 | 1.961 | 1.817 | 909 | 845 | 1.208 |
| Off | 4 | 7.076 | 2.216 | 1.123 | 1.095 | 1.458 |
| Off | 8 | 18.804 | 2.560 | 1.300 | 1.270 | 1.761 |
| On | 1 | 23 | 1.746 | 1.098 | 930 | 1.268 |
| On | 2 | 2.059 | 2.017 | 1.025 | 930 | 1.386 |
| On | 4 | 7.925 | 2.538 | 1.318 | 1.303 | 1.644 |
| On | 8 | 21.754 | 2.978 | 1.536 | 1.531 | 2.000 |

La espera medida crece marcadamente con los workers: a ocho workers representa
una mediana de 18,8 µs por adquisición sin snapshots y 21,8 µs con ellos. El
tiempo de la sección crítica también crece, de 1,54 µs a 2,56 µs y de 1,75 µs
a 2,98 µs, respectivamente. Los syscalls se ejecutan dentro de la sección
crítica en la implementación actual. Esto evidencia contención sobre la ruta
real, pero no identifica al mutex como causa única: la duración de syscall
incluye trabajo y esperas del kernel. Tampoco se deben sumar medianas de
componentes como si fueran una medición de la mediana del ciclo completo.

El snapshotter ejecutó una mediana de 1, 1, 2 y 4 snapshots por muestra para
1, 2, 4 y 8 workers. Cada lectura en torno a 200.000 VMA tardó una mediana
aproximada de 58–64 ms. Los snapshots ocurren fuera del Mutex global, aunque el
thread que solicita cada lectura soporta este coste.

## Resultado y límites de inferencia

- No hubo rechazos ni errores de asignación en los ciclos medidos.
- La VMA observada al comienzo y al final quedó en torno a 200.000; no se
  retuvieron los mappings temporales de los workers.
- El costo por syscall aumentó con la concurrencia, consistente con trabajo
  compartido/contención del kernel, pero la medición no separa el tiempo de
  CPU dentro del kernel ni atribuye causalidad exclusiva.
- El coste de instrumentación añade timers y atomics a la ruta. Los números
  localizan componentes; no sustituyen un benchmark sin instrumentación.
- El escenario no mide cientos de miles de stacks retenidos *además* de los
  workers (los 100k retenidos son el conjunto de base), ni scheduler, ejecución
  real de rutinas, context switching, pools o slabs.

## Estado F2 y decisión arquitectónica

Fixed Stack permanece como estrategia vigente y predeterminada. No se cambió
el allocator de producción, no se implementaron slabs ni lock-free y no se
presenta este workload como evaluación del scheduler. El ADR de esta decisión
es [ADR 008](../../decisions/008-allocator-contention-measurement.md).

F2 sigue abierta: las pruebas de fallo fatal de `munmap` durante `Drop` y de
limpieza tras fallo de protección existen. Las pruebas de integración cubren
ownership RAII, estabilidad de direcciones y no solapamiento. El overflow
ejecutándose en un stack real de rutina corresponde a F3. No hay pool/free-list
ni promesa de reutilización implementados; Dynamic Growable Stack continúa
experimental y no implementado.

## Reproducibilidad

Comando: `./scripts/bench-all.sh`. El runner completó todos los targets de
benchmark registrados y los benchmarks manuales ignorados de VMA; no modificó
el límite del sistema. El log bruto completo, con resultados por muestra y por
worker, está en
[20261114T132501Z-3150726.log](./runs/20261114T132501Z-3150726.log).
La metodología de cargas y limitaciones está en
[methodology.md](../methodology.md).

La comparación posterior conserva este archivo como evidencia histórica
independiente y contrasta el camino serial con el prototipo en dos fases en
[su informe dedicado](./2026-11-14-two-phase-stack-allocator.md).
