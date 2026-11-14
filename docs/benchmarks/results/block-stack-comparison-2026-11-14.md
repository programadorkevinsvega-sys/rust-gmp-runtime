# Comparación Fixed Stack vs. block/arena allocator

## Alcance y método

Se comparó el `StackManager` de producción con `BlockStackManager`, que solo
está disponible con `allocator-benchmark`. No se cambió el allocator ni el
default de producción. El host fue Linux x86_64, Rust 1.85.0, con
`vm.max_map_count=1.048.576`; ninguna escala se omitió por capacidad.

La ejecución final aplicó el método de snapshots controlados y el torneo de
tres allocators:

```sh
./scripts/bench-block-stack.sh
```

La salida completa final está en
[20261114T181152Z-block-stack-3818902.log](./runs/20261114T181152Z-block-stack-3818902.log).
Se conservan también las corridas anteriores: la inicial que halló el
recorrido cuadrático de slots
([142551](./runs/20261114T142551Z-block-stack-3396089.log)), la primera tras
añadir la lista libre ([143103](./runs/20261114T143103Z-block-stack-3411195.log)),
la comparación original completa ([143403](./runs/20261114T143403Z-block-stack-3422385.log))
y la primera corrida con snapshots sincronizados
([180757](./runs/20261114T180757Z-block-stack-3802825.log)). Esta última
sirve para la comparación concurrente controlada, pero no para comparar el
throughput sin instrumentar de `MADV_DONTNEED`: la primera versión del runner
no transmitía esa opción al constructor sin instrumentación. La opción quedó
corregida y la corrida final se repitió. No se promedian ejecuciones distintas.

La ejecución final hizo tres muestras por cardinalidad (1k, 10k, 100k y 500k),
diez muestras independientes de 20.000 ciclos de reutilización por variante y
cinco muestras por escenario concurrente. La reutilización tocó todas las
páginas útiles en cada ciclo. Cada muestra concurrente tuvo 100.000 stacks
retenidos, 2.000 ciclos por worker y 1/2/4/8 workers. El escenario con
snapshotter ejecutó exactamente cinco lecturas reales de `/proc/self/maps`,
sincronizadas con cinco lotes iguales de operaciones; sin snapshotter se
mantuvieron las mismas barreras por lote. Preflight VMA estuvo habilitado.
Fixed serial, Fixed dos fases y Arena se midieron con y sin instrumentación.
Los percentiles individuales son las medianas de los percentiles por muestra,
no percentiles de una unión artificial de muestras.

RSS se leyó desde `/proc/self/statm`. En las escalas de cardinalidad se tocó 1
de cada 100 slots y esas cifras son aproximadas; las pruebas de reutilización
tocaron cada página usable de todos los stacks.

## Reserva y liberación por escala

Medianas de tres muestras. Los tiempos de liberación incluyen destruir todos
los handles; `reserved` y RSS son observaciones mientras el conjunto seguía
vivo.

| Stacks vivos | Allocator | Reserva total | Reserva ops/s | Liberación total | VMAs vivas | Arenas | Slots libres | Memoria virtual reservada | Memoria accesible | RSS observado |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1.000 | Fixed | 3,04 ms | 329.242 | 1,24 ms | 2.031 | — | 0 | 12.288.000 B | 8.192.000 B | 2.678.784 B |
| 1.000 | Arena | 1,35 ms | 741.971 | 1,26 ms | 2.032 | 1 | 24 | 12.582.912 B | 8.192.000 B | 2.686.976 B |
| 10.000 | Fixed | 20,57 ms | 486.108 | 13,37 ms | 20.031 | — | 0 | 122.880.000 B | 81.920.000 B | 3.551.232 B |
| 10.000 | Arena | 17,18 ms | 582.030 | 16,20 ms | 20.032 | 10 | 240 | 125.829.120 B | 81.920.000 B | 3.653.632 B |
| 100.000 | Fixed | 190,00 ms | 526.307 | 119,82 ms | 200.031 | — | 0 | 1.228.800.000 B | 819.200.000 B | 12.902.400 B |
| 100.000 | Arena | 147,07 ms | 679.929 | 139,43 ms | 200.032 | 98 | 352 | 1.233.125.376 B | 819.200.000 B | 13.295.616 B |
| 500.000 | Fixed | 1.064,48 ms | 469.712 | 617,20 ms | 1.000.031 | — | 0 | 6.144.000.000 B | 4.096.000.000 B | 51.683.328 B |
| 500.000 | Arena | 1.128,53 ms | 443.053 | 691,31 ms | 1.000.032 | 489 | 736 | 6.153.043.968 B | 4.096.000.000 B | 56.139.776 B |

El experimento no redujo el número de VMAs cuando los stacks estaban vivos:
con 500.000 stacks ambos diseños alcanzaron aproximadamente 1.000.000 VMAs.
Los `mprotect` por slot fragmentan las arenas y anulan la reducción que podría
sugerir agrupar los `mmap`. Al terminar el conjunto, Fixed volvió a 32 VMAs;
Arena retuvo sus mappings y observó 10.033 VMAs hasta destruir el manager.
Arena también reservó unos 9 MB más de espacio virtual y tuvo mayor RSS
observado en el muestreo ligero de 500k.

Hasta 100k, Arena reservó más rápido en esta ejecución. A 500k fue
aproximadamente 5% más lento; no hay una ventaja general de throughput ni de
VMAs que justifique promoción.

## Reutilización

Se ejecutaron diez muestras con managers independientes por configuración.
Cada muestra calentó y liberó 1.024 slots; en los 20.000 ciclos medidos se
escribió en cada página usable antes de liberar el stack. La tabla muestra
medianas entre muestras y, entre paréntesis, el rango de throughput
desinstrumentado. Los percentiles de ciclo proceden de las muestras
instrumentadas.

| Allocator | `MADV_DONTNEED` | Throughput ops/s (rango) | p50 / p95 / p99 / máximo del ciclo |
|---|---:|---:|---:|
| Fixed serial | No aplica | 193.371 (135.295–227.636) | 4.848 / 9.152 / 11.841 / 184.717 ns |
| Fixed dos fases | No aplica | 199.253 (177.239–215.531) | 4.788 / 8.901 / 11.484 / 178.080 ns |
| Arena | No | 652.988 (630.938–688.708) | 2.034 / 2.129 / 2.440 / 107.670 ns |
| Arena | Sí | 252.106 (144.963–274.039) | 4.210 / 8.080 / 8.840 / 172.038 ns |

Arena sin advice obtuvo unas 3,3 veces el throughput de Fixed en este ciclo
local de reutilización. Activar `MADV_DONTNEED` redujo la mediana a 252k
ops/s, por debajo de Arena sin advice, y aumentó las latencias instrumentadas.
En cambio, después del calentamiento de 1.024 stacks completamente tocados, el
RSS observado fue 44.052.480 B sin advice y 35.663.872 B con advice (igual al
de Fixed): recuperó unos 8 MiB, el tamaño aproximado de las páginas útiles
calentadas. En esta carga concreta, advice ofrece una recuperación medible de
RSS a cambio de throughput; su conveniencia depende de la presión de memoria.

## Ciclo concurrente con 100.000 stacks retenidos

Las tasas siguientes son medianas de cinco muestras sin instrumentación:

| Workers | Fixed serial, sin snapshots | Fixed dos fases, sin snapshots | Arena, sin snapshots | Fixed serial, 5 snapshots | Fixed dos fases, 5 snapshots | Arena, 5 snapshots |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 317.999 | 353.656 | 333.581 | 6.856 | 8.197 | 8.183 |
| 2 | 249.362 | 283.518 | 310.723 | 12.893 | 16.718 | 16.164 |
| 4 | 172.717 | 181.902 | 313.649 | 26.225 | 32.113 | 29.858 |
| 8 | 161.125 | 167.206 | 240.455 | 57.746 | 55.323 | 51.083 |

Unidades: ops/s. Con snapshots, el runner fuerza exactamente cinco lecturas
por muestra y guarda los cinco tiempos y conteos de VMA; sin snapshots se
mantienen las mismas barreras de lote. El resultado previo (318k Fixed frente
a 30k Arena con un worker, y distintos conteos de snapshots) no era comparable
y se descarta para inferir la brecha de throughput. Con cinco snapshots
equivalentes, Arena no pierde 76–90%: a ocho workers queda 11,5% bajo Fixed
serial y 7,7% bajo Fixed dos fases. A 1–4 workers las tasas son cercanas y
dominadas por el tiempo de las cinco lecturas.

Los tiempos individuales de snapshot en las muestras sin instrumentación (25
lecturas por combinación estrategia/workers) fueron:

| Allocator | Workers | p50 de lectura | p95 de lectura | Máximo |
|---|---:|---:|---:|---:|
| Fixed serial | 1 | 57,13 ms | 61,80 ms | 73,12 ms |
| Fixed serial | 2 | 60,90 ms | 67,50 ms | 95,15 ms |
| Fixed serial | 4 | 60,80 ms | 67,36 ms | 69,59 ms |
| Fixed serial | 8 | 55,28 ms | 56,58 ms | 61,58 ms |
| Fixed dos fases | 1 | 48,08 ms | 51,37 ms | 54,28 ms |
| Fixed dos fases | 2 | 47,58 ms | 49,86 ms | 49,92 ms |
| Fixed dos fases | 4 | 49,76 ms | 52,36 ms | 53,91 ms |
| Fixed dos fases | 8 | 57,51 ms | 62,72 ms | 64,89 ms |
| Arena | 1 | 47,66 ms | 52,89 ms | 55,84 ms |
| Arena | 2 | 47,18 ms | 49,25 ms | 50,77 ms |
| Arena | 4 | 48,09 ms | 50,11 ms | 50,93 ms |
| Arena | 8 | 50,68 ms | 56,92 ms | 93,42 ms |

Percentiles instrumentados del ciclo `allocate() -> Drop`; son medianas entre cinco muestras:

| Allocator | Snapshots | Workers | p50 ns | p95 ns | p99 ns | Máximo ns |
|---|---:|---:|---:|---:|---:|---:|
| Fixed serial | No | 1 | 4.519 | 6.414 | 10.230 | 89.934 |
| Fixed serial | No | 2 | 10.645 | 19.246 | 26.754 | 302.860 |
| Fixed serial | No | 4 | 27.280 | 58.222 | 89.966 | 873.661 |
| Fixed serial | No | 8 | 59.542 | 129.891 | 220.342 | 812.965 |
| Fixed serial | Sí | 1 | 5.226 | 8.410 | 11.770 | 108.387 |
| Fixed serial | Sí | 2 | 7.521 | 14.896 | 22.297 | 164.220 |
| Fixed serial | Sí | 4 | 20.616 | 40.044 | 96.238 | 672.976 |
| Fixed serial | Sí | 8 | 49.724 | 103.186 | 166.497 | 732.024 |
| Fixed dos fases | No | 1 | 2.688 | 5.364 | 6.896 | 28.241 |
| Fixed dos fases | No | 2 | 5.462 | 10.064 | 12.823 | 147.469 |
| Fixed dos fases | No | 4 | 18.087 | 24.432 | 30.426 | 840.184 |
| Fixed dos fases | No | 8 | 47.452 | 59.493 | 92.948 | 2.479.944 |
| Fixed dos fases | Sí | 1 | 5.080 | 7.840 | 11.097 | 128.411 |
| Fixed dos fases | Sí | 2 | 7.653 | 11.977 | 15.463 | 148.479 |
| Fixed dos fases | Sí | 4 | 18.011 | 24.212 | 28.251 | 173.505 |
| Fixed dos fases | Sí | 8 | 46.936 | 58.264 | 68.436 | 4.042.760 |
| Arena | No | 1 | 3.754 | 6.695 | 7.922 | 145.918 |
| Arena | No | 2 | 3.568 | 8.915 | 11.446 | 160.998 |
| Arena | No | 4 | 12.311 | 18.374 | 24.590 | 185.355 |
| Arena | No | 8 | 31.573 | 45.621 | 62.001 | 721.052 |
| Arena | Sí | 1 | 2.211 | 4.671 | 6.181 | 53.318.137 |
| Arena | Sí | 2 | 5.732 | 9.306 | 13.360 | 47.727.325 |
| Arena | Sí | 4 | 11.972 | 17.946 | 23.002 | 51.614.954 |
| Arena | Sí | 8 | 28.007 | 40.195 | 49.976 | 52.378.427 |

Los tiempos de snapshot se calcularon sobre 25 lecturas por combinación en
muestras sin instrumentación. Los percentiles por operación y los máximos
corresponden a muestras instrumentadas separadas; Arena presenta máximos de
decenas de milisegundos cuando coincide con una lectura.

## Mutex y syscalls en el ciclo real

Promedio de los contadores instrumentados por ciclo completo
`allocate() -> Drop`, acumulado sobre las cinco muestras. Los valores están en
nanosegundos por operación y son observacionales: la instrumentación agrega
overhead.

| Allocator | Snapshots | Workers | Espera Mutex | Espera Condvar | Posesión Mutex | `mmap` | `mprotect` | `munmap` |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Fixed serial | No | 1 | 51 | 0 | 3.831 | 1.193 | 959 | 1.385 |
| Fixed serial | No | 8 | 48.841 | 0 | 6.947 | 1.797 | 1.771 | 2.177 |
| Fixed serial | Sí | 1 | 93 | 0 | 5.207 | 1.674 | 1.411 | 1.776 |
| Fixed serial | Sí | 8 | 40.436 | 0 | 5.812 | 1.510 | 1.518 | 1.825 |
| Fixed dos fases | No | 1 | 69 | 0 | 183 | 958 | 777 | 1.156 |
| Fixed dos fases | No | 8 | 136 | 0 | 293 | 14.389 | 14.961 | 16.790 |
| Fixed dos fases | Sí | 1 | 78 | 0 | 201 | 1.322 | 1.146 | 1.427 |
| Fixed dos fases | Sí | 8 | 125 | 0 | 270 | 14.396 | 15.169 | 16.666 |
| Arena | No | 1 | 123 | 0 | 604 | 0 | 2.314 | 0 |
| Arena | No | 8 | 327 | 0 | 1.091 | 0 | 30.872 | 0 |
| Arena | Sí | 1 | 106 | 121.097 | 440 | 0 | 1.751 | 0 |
| Arena | Sí | 8 | 347 | 127.240 | 935 | 0 | 26.784 | 0 |

Valores en ns/op, agregados entre muestras instrumentadas sin y con snapshots
controlados; la tabla resume workers 1 y 8. Para Fixed las rutas no realizan
espera de `Condvar`. Para Arena, `lock_wait_ns` mide solo la adquisición del
Mutex y `condvar_wait_ns` se muestra por separado: las esperas observadas
previamente de hasta 168,3 µs no eran espera real del Mutex, sino espera de
refresh mal etiquetada. En Arena, las arenas se reservan antes del ciclo y el
`munmap` ocurre cuando muere el último owner, por eso esos contadores de ciclo
son cero. Al cerrar el conjunto concurrente se observaron 98 `munmap` de
arenas, con 2.184.200 ns acumulados. En la prueba con ocho workers y snapshots,
la espera media de `Condvar` fue ~127 µs/op, que explica la penalización de
Arena sin implicar que mantenga el Mutex durante la lectura de `/proc/self/maps`.

## Primera ejecución, corrección y decisión

La primera corrida mostró que `take_free_slot` buscaba desde el inicio de
cada arena en cada asignación. Con muchas arenas, ese recorrido repetido
dominaba la escala de 500k (32,6 s). Se cambió el prototipo aislado a una lista
de slots disponibles por arena, actualizada al liberar o revertir una
asignación. La segunda corrida, registrada arriba, redujo la reserva de 500k
de 32,6 s a 1,23 s. La primera salida se conserva para hacer auditable el
hallazgo, pero no describe el código final.

La lista disponible se mantiene solo en el allocator experimental. Fixed Stack
no se tocó; no se añadieron slabs de producción ni crecimiento automático.
El experimento permanece opt-in y no cierra F2 ni inicia F3.

## Riesgos y pendientes

- El prototipo no reduce VMAs con permisos por slot; a cardinalidad alta,
  persiste el límite de `max_map_count`.
- Con cinco snapshots controlados, Arena queda cerca de Fixed en throughput,
  aunque a ocho workers es 11,5% más lento que Fixed serial. La espera de
  `Condvar` durante el refresh explica la demora; mejorar esa política sigue
  pendiente y no forma parte de este experimento.
- El RSS de las escalas es una muestra parcial porque solo se tocaron slots
  dispersos; la prueba de reutilización sí tocó todas las páginas útiles.
- La reutilización tiene diez muestras independientes por condición; sus
  rangos están documentados y conviene repetirlos en otros hosts antes de
  generalizar el resultado.
- Hay pruebas de fallo para reserva, `mprotect`, rollback, cuarentena y
  `madvise`. El fallo de `munmap` de las arenas no tiene inyección específica
  dentro del prototipo; su limpieza sigue la política fatal de `Mapping`.
- No se midieron rutinas reales, scheduler, context switching ni crecimiento
  dinámico.

## Archivos añadidos o modificados

- Manifiesto y módulos del runtime:
  [Cargo.toml](../../../Cargo.toml),
  [stack/mod.rs](../../../src/stack/mod.rs) y
  [block_experimental.rs](../../../src/stack/block_experimental.rs).
- Prototipo de arena:
  [api.rs](../../../src/stack/block_experimental/api.rs),
  [allocation.rs](../../../src/stack/block_experimental/allocation.rs),
  [arena.rs](../../../src/stack/block_experimental/arena.rs),
  [budget.rs](../../../src/stack/block_experimental/budget.rs),
  [failure.rs](../../../src/stack/block_experimental/failure.rs),
  [lifecycle.rs](../../../src/stack/block_experimental/lifecycle.rs),
  [manager.rs](../../../src/stack/block_experimental/manager.rs),
  [memory.rs](../../../src/stack/block_experimental/memory.rs),
  [release.rs](../../../src/stack/block_experimental/release.rs),
  [state.rs](../../../src/stack/block_experimental/state.rs) y
  [timing.rs](../../../src/stack/block_experimental/timing.rs).
- Constructor Fixed dos fases para benchmarks:
  [allocator_benchmarked.rs](../../../src/stack/allocator_benchmarked.rs).
- Pruebas:
  [tests.rs](../../../src/stack/block_experimental/tests.rs),
  [concurrency.rs](../../../src/stack/block_experimental/tests/concurrency.rs),
  [failures.rs](../../../src/stack/block_experimental/tests/failures.rs) y
  [layout.rs](../../../src/stack/block_experimental/tests/layout.rs).
- Benchmarks:
  [block_stack_compare.rs](../../../benches/block_stack_compare.rs),
  [common.rs](../../../benches/block_stack_compare/common.rs),
  [concurrency.rs](../../../benches/block_stack_compare/concurrency.rs),
  [lifecycle.rs](../../../benches/block_stack_compare/lifecycle.rs),
  [stats.rs](../../../benches/block_stack_compare/stats.rs) y
  [workers.rs](../../../benches/block_stack_compare/workers.rs).
- Ejecución reproducible:
  [bench-block-stack.sh](../../../scripts/bench-block-stack.sh) y
  [bench-all.sh](../../../scripts/bench-all.sh).
- Documentación:
  [ADR 010](../../decisions/010-block-stack-experiment.md),
  [ADR 011](../../decisions/011-controlled-allocator-comparison.md),
  [diseño del allocator](../../block-stack-allocator.md) y este informe.
- Las tres salidas brutas independientes están enlazadas al comienzo del
  informe.

## Verificación

- `cargo +1.85.0 check --all-targets --features allocator-benchmark`: pasó
  después de los ajustes.
- `cargo +1.85.0 test --lib --features allocator-benchmark`: pasó después del
  ajuste; 58 pasaron y dos benchmarks manuales fueron ignorados.
- `cargo +1.85.0 clippy --all-targets --features allocator-benchmark -- -D warnings`:
  pasó después del ajuste.
- `cargo +1.85.0 fmt --all -- --check`: pasó después de actualizar este
  informe.
- `./scripts/check-forbidden.sh`: no pasó en esta verificación; reporta que
  `benches/block_stack_compare/common.rs` tiene 218 líneas frente al máximo
  configurado de 200. No se modificó ese archivo como parte de esta
  actualización documental.
- Los enlaces relativos de este informe se comprobaron y todos resuelven.
- `./scripts/bench-block-stack.sh`: completó ambas corridas; las dos salidas
  están preservadas en `runs/`.

**Dictamen:** mantener Fixed Stack como allocator de producción. El prototipo
ofrece ventajas en ciertos ciclos de reutilización y concurrencia sin
snapshots, pero no mejora VMAs ni gana a 500k en la reserva fría. Bajo cinco
snapshots controlados, Arena queda cerca de Fixed y pierde 11,5% frente a
Fixed serial a ocho workers; el contador corregido atribuye la espera larga a
la `Condvar` del refresh, no al Mutex. No se recomienda promoverlo ni
implementar slabs productivos con esta evidencia.
