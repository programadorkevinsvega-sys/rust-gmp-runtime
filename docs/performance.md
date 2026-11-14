# Rendimiento

## Método y comparabilidad

Primero establecer una línea base funcional y reproducible; luego perfilar,
identificar cuellos de botella y cambiar una cosa a la vez. No introducir
métricas objetivo ni afirmaciones de superioridad sin datos. La comparación
con threads nativos de Rust solo se hace con cargas semánticamente equivalentes;
reportar cuando una operación no tenga equivalente.

Los benchmarks del runtime usarán ejecutables `harness = false` y
`std::time::Instant`, sin crates de benchmark. Por workload registrar al menos
10 ejecuciones, calentamiento, mediana, p99 y dispersión; usar el mismo número
de workers en la comparación. Guardar commit, `rustc -V`, CPU, kernel, governor,
comando exacto, configuración, código del workload y resultados bajo
`docs/benchmarks/results/<fecha>-<commit>.md`. No inventar resultados; reportar
si faltan datos o condiciones comparables.

Comparar cargas equivalentes con el runtime propio y threads nativos de Rust:
CPU pequeña, espera, canales, carga mixta, miles de tareas, red y carga
sostenida. Registrar hardware, SO, toolchain, comandos, repeticiones, variación
y diferencias semánticas.

## Métricas

Spawn, ejecución, latencia/tiempo de planificación, context switch, throughput, CPU, espera, contención, tareas por P, longitud de colas, asignaciones/liberaciones, memoria virtual reservada, comprometida y RSS, RAM por tarea y total. Distinguir resultados observados de estimaciones.

Para stacks, medir además número de stacks/slabs/mapeos/VMA, páginas
comprometidas/tocadas, fragmentación, reutilización, fallos de asignación y
latencia de reserva/liberación. Separar RAM residente de espacio virtual.

## Prioridades

Corregir primero. Evitar busy waiting, despertar workers innecesarios, locks
globales largos, copias y retención ilimitada. No sustituir colas sincronizadas
ni allocator hasta que mediciones lo justifiquen. Ver
[benchmark methodology](./benchmarks/methodology.md).
