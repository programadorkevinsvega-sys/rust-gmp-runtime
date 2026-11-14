# Recuperación de spans y contención de refills

## Alcance

Medición experimental de la ruta de recuperación del allocator Go-inspired,
sin cambiar Fixed Stack como implementación vigente. Se ejecutó el torneo
completo de cuatro allocators en Linux x86_64, Rust 1.85.0, kernel 7.0.0-42 y
`vm.max_map_count=1.048.576`. El host tenía 1.048.531 mappings de margen y el
runner conservó su preflight. La
[salida bruta](./runs/20261114T200136Z-allocator-tournament-23473.log) contiene
las 160 muestras de escala, 100 de reutilización, 640 concurrentes, 1.600
snapshots y 160 muestras de caché/refill.

La fase de escala Go-inspired mantuvo las guard pages; después de liberar los
handles, creó ocho cachés de worker, dejó slots locales en ellas y coordinó su
vaciado con la liberación de spans vacíos. Cada cifra resume diez muestras.

| Stacks | VMAs y bytes reservados antes de recuperar | Spans liberados | Recuperación mediana | VMAs y bytes reservados después |
|---:|---:|---:|---:|---:|
| 1.000 | 2.032; 12 MiB | 1 | 0,60 ms | 32; 0 |
| 10.000 | 20.032; 120 MiB | 10 | 7,67 ms | 32; 0 |
| 100.000 | 200.032; 1,15 GiB | 98 | 88,63 ms | 32; 0 |
| 500.000 | 1.000.032; 5,73 GiB | 489 | 503,32 ms | 32; 0 |

El espacio virtual reservado por spans llegó a cero en todos los puntos. El RSS
observado tras recuperar los 500k pasó de 60,3 MiB a 39,8 MiB; el proceso seguía
vivo y el RSS no se atribuye exclusivamente al allocator. Recuperar mappings
requiere leer `/proc/self/maps` para las métricas y ejecutar `munmap` por span:
el coste crece con la escala y esta ruta es mantenimiento explícito, no trabajo
gratuito de `Drop`.

## Caché caliente y refill forzado

La concurrencia mantuvo 100.000 stacks vivos. El escenario forzado devolvió los
slots locales al pool global en cada ciclo para hacer que la siguiente
asignación tuviera que pasar por refill. La tabla da throughput mediano de diez
muestras en miles de operaciones/s; el orden de workers es 1 / 2 / 4 / 8.

| Instrumentación | Caché caliente | Refill forzado |
|---|---:|---:|
| Desactivada | 4.438 / 3.363 / 3.441 / 3.059 | 2.745 / 1.706 / 1.540 / 1.153 |
| Activada | 4.851 / 3.235 / 3.350 / 3.183 | 1.530 / 1.478 / 1.097 / 0.760 |

La caché caliente retuvo el camino rápido; forzar un flush por ciclo redujo el
throughput, especialmente con más workers. En las muestras forzadas
instrumentadas, el lock global acumuló:

| Workers | Refills | Refills solapados | Pico concurrente | Espera total del lock | Posesión total del lock |
|---:|---:|---:|---:|---:|---:|
| 1 | 20.000 | 0 | 1 | 1,05 ms | 2,87 ms |
| 2 | 40.000 | 19.917 | 2 | 16,75 ms | 12,82 ms |
| 4 | 80.000 | 65.218 | 4 | 158,76 ms | 25,16 ms |
| 8 | 160.000 | 157.051 | 8 | 1.104,14 ms | 73,72 ms |

Cada refill transfirió un slot en esta prueba deliberadamente extrema. No se
registraron llamadas `mmap`, `mprotect` ni `munmap`, ni fallos de asignación o
limpieza en las muestras del allocator. Los valores con instrumentación no son
throughput puro. Los snapshots concurrentes se midieron en la matriz normal de
100k stacks retenidos y cinco lecturas por muestra; el escenario que fuerza
refill se ejecutó sin snapshots y no caracteriza aún la interacción refill +
snapshot en alta cardinalidad.

## Seguridad, fallos e integración futura

- Las pruebas ejercitan leases activos y slots en caché como bloqueadores de
  trim, vaciado coordinado, retención configurable y liberación completa.
- La falla inyectada de `munmap` verifica que el mapping/contabilidad se
  restauran, que el fallo de cleanup se registra y que un segundo intento puede
  recuperar el span.
- El refill vuelve a intentar tomar slots si otro worker consume el lote que
  acaba de crearse entre la creación del span y la toma del slot. La prueba
  concurrente de leases distintos pasó cinco ejecuciones repetidas.
- `flush_all_caches` exige que el llamador suministre las cachés pertenecientes
  al manager en un punto seguro. La ruta no recorre cachés implícitamente ni
  asume Processors reales; el protocolo de quiescencia de P queda como requisito
  de integración del scheduler.
- El experimento demuestra recuperación después de que los stacks quedan
  inactivos. No reduce la cardinalidad de VMA durante una carga activa, no
  elimina guard pages y no justifica promocionar el allocator.

La decisión y sus límites están registrados en
[ADR 013](../../decisions/013-span-reclamation.md). El benchmark completo es
reproducible con [`bench-go-inspired-stack.sh`](../../scripts/bench-go-inspired-stack.sh).

## Verificación

- `cargo fmt --manifest-path rust-gmp-runtime/Cargo.toml --check`: correcto.
- `cargo test --manifest-path rust-gmp-runtime/Cargo.toml --features allocator-benchmark --lib -- --test-threads=1`: 73 pasaron, 2 ignoradas.
- `cargo check --manifest-path rust-gmp-runtime/Cargo.toml --benches --features allocator-benchmark`: correcto.
- `cargo clippy --manifest-path rust-gmp-runtime/Cargo.toml --all-targets --features allocator-benchmark`: correcto; muestra warnings preexistentes de `fetch_update` obsoleto y `manual_ok_err` fuera de esta ruta.
- `./rust-gmp-runtime/scripts/bench-go-inspired-stack.sh`: torneo completo finalizado correctamente, sin fallos de asignación ni cleanup en las muestras.
