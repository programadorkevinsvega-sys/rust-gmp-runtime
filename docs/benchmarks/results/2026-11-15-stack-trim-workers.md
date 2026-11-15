# Retención y trim Go-inspired con workers reales

## Objetivo y método

Se comprobó la recuperación de spans con stacks de 64 KiB bajo ejecución
stackful real. Cada muestra corre en un proceso nuevo con un único
`MachineManager` de 1, 2, 4, 8, 12 o 16 workers. El proceso precrea 1.024
`Routine` antes del dispatch, de modo que todos los leases coexisten como en una
ráfaga; cada closure se ejecuta en su stack, se hace join y se valida una
ejecución por rutina. Luego se toman snapshots, se llama `trim()` (retención
predeterminada de un span), y finalmente `trim_to_retained_spans(0)`.

El `StackManager` usa el backend Go-inspired, pool global y spans de 64 slots.
La creación de rutinas/leases ocurre en el caller; los workers son reales para
dispatch y ejecución, no se afirma afinidad de caches de allocator a Processors.
Hay tres procesos independientes por número de workers. El benchmark valida
conteos y estados de memoria, pero sus duraciones no se usan como comparación
de rendimiento. El contador exactly-once añade una operación atómica por
closure. No se fijó afinidad de CPU.

Entorno: Rust 1.85.0, Linux x86_64, kernel `7.0.0-42-generic`, 12 CPUs lógicas.
Comando: `cargo +1.85.0 bench --bench stack_trim_workers`.
Datos por muestra:
[`20261115-stack-trim-workers.log`](./runs/20261115-stack-trim-workers.log).

## Geometría y resultados

Con páginas de 4 KiB, cada slot ocupa 69.632 bytes virtuales (64 KiB útiles y
una guard page), así que 64 slots reservan 4.456.448 bytes (4,25 MiB). Para
1.024 rutinas simultáneas, todos los workers y las tres repeticiones observaron
16 spans: `16 × 4.456.448 = 71.303.168` bytes reservados (68 MiB) y
`1.024 × 65.536 = 67.108.864` bytes accesibles (64 MiB).

| Workers | VMAs base* | VMAs antes del trim | `trim()` libera spans | Reservado tras `trim()` | Accesible tras `trim()` | VMAs tras `trim()` | RSS antes → tras `trim()` (bytes, mediana) | `trim()` mediana |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 39 | 2.086 | 15 | 4.456.448 | 4.194.304 | 167 | 7.974.912 → 4.042.752 | 1,25 ms |
| 2 | 43 | 2.090 | 15 | 4.456.448 | 4.194.304 | 171 | 8.146.944 → 4.214.784 | 1,22 ms |
| 4 | 51 | 2.098 | 15 | 4.456.448 | 4.194.304 | 179 | 8.589.312 → 4.657.152 | 1,24 ms |
| 8 | 67 | 2.114 | 15 | 4.456.448 | 4.194.304 | 195 | 9.138.176 → 5.206.016 | 1,27 ms |
| 12 | 83 | 2.130 | 15 | 4.456.448 | 4.194.304 | 211 | 9.760.768 → 5.828.608 | 1,21 ms |
| 16 | 100 | 2.147 | 15 | 4.456.448 | 4.194.304 | 228 | 10.059.776 → 6.127.616 | 1,78 ms |

`trim()` liberó exactamente 15 spans en las 18 muestras y conservó uno, según
la política predeterminada. Quedaron 4 MiB de páginas accesibles dentro del
span retenido. El recorte total posterior liberó ese span: reservado y
accesible pasaron a cero, y en todas las muestras el VMA del proceso volvió al
valor base medido con los workers todavía vivos. El tiempo mediano del trim
total estuvo entre 0,060 y 0,085 ms. Así, la liberación funciona con la ráfaga
de 1.024 rutinas y workers reales.

El resultado reproduce la preocupación por VMA: antes del trim hubo de
2.086 a 2.147 VMA; incluso después de liberar 15 spans, conservar uno dejó de
exactamente 128 VMA por encima de la base de cada proceso. Al liberar también
el span retenido se vuelve a la base. Por tanto, `trim()` no significa “sin
retención”: la API conserva deliberadamente un span; el caller puede solicitar
`trim_to_retained_spans(0)` cuando quiere vaciar el pool. El experimento no
demuestra que el `MachineManager` invoque trim automáticamente ni que exista una
política automática de presión de memoria.

Los 64 MiB accesibles son espacio virtual con permisos habilitados, no RSS. El
RSS del proceso bajó tras el trim, pero incluye workers, runtime y otras
asignaciones; no se atribuye íntegramente a stacks. La conclusión fuerte aquí
es sobre bytes virtuales reservados y VMAs observados, no sobre memoria física
ahorrada ni sobre causalidad de throughput.

*La base de VMA se mide después de iniciar el manager y sus workers, antes de
crear las rutinas. El conteo es del proceso completo.
