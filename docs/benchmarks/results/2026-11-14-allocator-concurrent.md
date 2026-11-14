# Benchmark concurrente del allocator de stacks

## Alcance y entorno

Se midió el `StackManager` Fixed Stack actual en Linux x86_64 con Rust 1.85.0.
El límite de VMA del host era 1.048.576; `scripts/bench-all.sh` observó 45 VMA
en su proceso y 1.048.531 de margen antes de iniciar. El entorno de CPU y
kernel coincide con el registrado en la [auditoría VMA](./2026-11-14-vma-performance.md).

El benchmark `allocator_concurrent` ejecuta diez muestras por nivel de
concurrencia. Cada worker realiza 5.000 ciclos reales de `allocate()` seguido
inmediatamente por `Drop`; un thread adicional solicita snapshots VMA durante
la muestra a intervalos de 1 ms. El tiempo de pared mide el lote de workers;
la latencia por operación incluye reserva y liberación. Las VMA y métricas del
manager se leen fuera de ese tramo cronometrado.

## Resultados

| Workers | Operaciones por muestra | Throughput mediano (ops/s) | Latencia p50 (ns) | Latencia p99 (ns) | Máximo observado (ns) | Dispersión de pared (ns) |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 5.000 | 395.468 | 2.065 | 7.497 | 429.408 | 9.508.824 |
| 2 | 10.000 | 261.202 | 3.397 | 29.669 | 2.112.551 | 11.142.898 |
| 4 | 20.000 | 242.180 | 5.756 | 84.082 | 1.659.515 | 23.371.545 |
| 8 | 40.000 | 244.117 | 5.354 | 196.219 | 1.837.161 | 30.044.074 |

El throughput cae al aumentar de uno a varios workers y se mantiene casi plano
entre cuatro y ocho. La latencia p99 y la dispersión también aumentan con la
contención. Esto caracteriza el allocator actual; no es una medición de
goroutines ejecutables, del scheduler ni de context switching.

No hubo errores de asignación ni rechazos preventivos. Al final de cada muestra
no quedaban mappings activos del manager (`runtime_mapping_count=0`,
`runtime_vma_estimate=0`, `reserved_virtual_bytes=0`). Las VMA observadas del
proceso variaron durante la ejecución; no se interpretan como mappings
atribuibles al runtime.

El p99 resume las latencias individuales agrupadas de diez muestras por nivel,
y no es una garantía ni una estimación de cola robusta bajo otros workloads.
Los ciclos liberan cada stack inmediatamente, por lo que este benchmark no
mide concurrencia con grandes conjuntos de stacks retenidos.

La [medición posterior de alta cardinalidad](./2026-11-14-allocator-high-cardinality.md)
mantiene 100.000 stacks vivos y añade contadores por worker para el Mutex real
y los syscalls. Se conserva este informe como referencia histórica
independiente: no se mezclan sus ciclos inmediatos ni sus resultados con el
workload de alta cardinalidad.

## Ejecución completa

Comando: `./scripts/bench-all.sh`. El script ejecutó los cinco targets
registrados (`allocator_concurrent`, `memory`, `routine_lifecycle`, `scale` y
`mapping_layout`), incluida la escala con protección preventiva, y luego ambos
tests manuales ignorados de VMA. Los dos perfiles manuales terminaron
satisfactoriamente; el test de alto VMA observó cerca de 800.040 regiones. La
escala de 1M se sometió al preflight y no se forzó porque excedía el
presupuesto. La ejecución no modificó límites globales.

La [salida bruta completa](./runs/20261114T125938Z-3069942.log) conserva las
40 muestras individuales, métricas VMA, refreshes, benchmarks seriales y la
salida de los dos perfiles manuales. Las líneas de cada muestra deben
consultarse junto con la tabla de resumen de este informe.

## Decisión y limitaciones

No se cambió la serialización del allocator ni se adoptaron slabs o Dynamic
Growable Stack. La reducción de throughput con más workers justifica perfilar
por separado mutex, syscalls y procfs antes de proponer una modificación
arquitectónica. El refresh concurrente en este workload fue solicitado
periódicamente, pero no todos los ciclos necesariamente coinciden con una
lectura lenta de alta cardinalidad.

El benchmark se repite mediante el [runner completo](../../../scripts/bench-all.sh)
y está descrito en la [metodología](../methodology.md). F2 permanece abierta;
estos resultados no aprueban su cierre ni modifican los pendientes de overflow
real sobre el stack de rutina, pool/reutilización o Dynamic Growable Stack.
