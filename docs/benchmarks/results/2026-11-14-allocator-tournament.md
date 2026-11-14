# Torneo comparativo de allocators de stacks

## Alcance y entorno

Comparación experimental de Fixed serial, Fixed dos fases, Block/Arena y
Go-inspired. No modifica el allocator de producción: Fixed Stack sigue siendo
la implementación vigente.

- Rust 1.85.0; Linux 7.0.0-42-generic; x86_64; 12 CPUs lógicos.
- `vm.max_map_count`: 1.048.576; 45 VMAs observadas antes de iniciar.
- Stack usable de 8 KiB; arenas/spans de hasta 1.024 slots.
- Diez muestras por punto de escala (1k, 10k, 100k, 500k), diez muestras de
  reutilización y diez muestras concurrentes por combinación.
- Concurrencia: 100.000 stacks retenidos; 1, 2, 4 y 8 workers; snapshots
  activados y desactivados.
- La preparación de la cache de cada worker queda fuera del cronómetro. En las
  muestras con snapshots cada worker ejecuta al menos un ciclo antes de
  sincronizar el snapshotter.
- El preflight se mantuvo habilitado. No se modificó el límite VMA del host.

La ejecución produjo 160 muestras de escala, 100 de reutilización, 640 de
concurrencia y 1.600 lecturas VMA. No hubo escalas omitidas ni fallos de
asignación/limpieza en los contadores disponibles. Go-inspired no expone
contadores de fallos equivalentes, por lo que su ausencia no se interpreta
como un contador igual a cero.

La [salida bruta completa](./runs/20261114T192110Z-allocator-tournament-4089589.log)
es la fuente de las cifras y permite recalcular cada mediana.

## Reserva fría y VMAs

Medianas de diez muestras para 500.000 stacks:

| Motor | Reserva (ops/s) | Tiempo de reserva | Tiempo de liberar handles | VMAs después de liberar handles |
|---|---:|---:|---:|---:|
| Fixed serial | 390.767 | 1.279,5 ms | 647,5 ms | 32 |
| Fixed dos fases | 346.680 | 1.442,3 ms | 650,9 ms | 32 |
| Block/Arena | 419.540 | 1.191,8 ms | 736,1 ms | 10.033 |
| Go-inspired | 621.197 | 804,9 ms | 39,3 ms | 1.000.032 |

El rango de throughput de reserva observado fue 349.317–444.192 ops/s en Fixed
serial, 344.360–377.113 en Fixed dos fases, 377.724–428.033 en Block y
568.550–637.387 en Go-inspired.

Durante la reserva, los cuatro motores llegaron a aproximadamente 1.000.031 o
1.000.032 VMAs. Agrupar slots en spans no redujo las VMAs: las protecciones por
slot siguen fragmentando los mappings. Block conserva sus arenas vacías; el
manager Go-inspired conserva sus spans y las protecciones de slot después de
liberar los handles. Go-inspired retuvo aproximadamente 6,15 GB de espacio
virtual hasta destruir el manager. Tras destruir cada manager, la escala
observó 32 VMAs.

El resultado de 100.000 stacks también favoreció a Go-inspired: mediana de
664.694 ops/s, frente a 598.405 Block, 453.618 Fixed dos fases y 406.174 Fixed
serial. La ventaja de reserva no elimina el coste de memoria virtual retenida.

## Reutilización con páginas tocadas

Throughput mediano no instrumentado y percentiles medianos de la ejecución
instrumentada, por ciclo:

| Motor | MADV_DONTNEED | Throughput (ops/s) | p50 (ns) | p95 (ns) | p99 (ns) | Mediana de máximos (ns) |
|---|---:|---:|---:|---:|---:|---:|
| Fixed serial | No | 214.050 | 4.577 | 8.465 | 10.111 | 164.202 |
| Fixed dos fases | No | 212.216 | 4.674 | 8.383 | 9.539 | 146.570 |
| Block/Arena | No | 686.996 | 1.994 | 2.024 | 2.145 | 78.296 |
| Block/Arena | Sí | 268.907 | 4.028 | 7.775 | 8.094 | 87.278 |
| Go-inspired | No | 5.126.651 | 355 | 364 | 419 | 76.954 |

La reutilización local Go-inspired es muy rápida en este workload concreto.
`MADV_DONTNEED` redujo el throughput de reutilización de Block/Arena; no se
justifica activarlo por defecto a partir de estas mediciones.

## Ciclos concurrentes con 100.000 stacks retenidos

Throughput mediano no instrumentado (ops/s). En las muestras sin snapshots,
cada worker completa la cuota de 2.000 ciclos. Con snapshots, continúa hasta
terminar las cinco lecturas; por ello, el número real de operaciones puede ser
mayor que esa cuota.

| Motor | Snapshots apagados: 1 / 2 / 4 / 8 workers | Snapshots encendidos: 1 / 2 / 4 / 8 workers |
|---|---:|---:|
| Fixed serial | 286.089 / 213.521 / 180.864 / 164.430 | 289.699 / 173.095 / 164.326 / 144.234 |
| Fixed dos fases | 349.690 / 327.953 / 204.694 / 152.202 | 328.846 / 247.375 / 175.163 / 146.004 |
| Block/Arena | 425.150 / 315.347 / 277.431 / 219.149 | 7.548 / 14.723 / 25.634 / 46.458 |
| Go-inspired | 5.417.868 / 3.041.522 / 2.789.939 / 2.420.373 | 6.741.741 / 3.011.595 / 2.389.837 / 2.361.283 |

Cada muestra con snapshots tomó exactamente cinco lecturas reales. Sus
duraciones p50/p95/máximo, sobre 400 lecturas por motor, fueron:

| Motor | p50 | p95 | Máximo |
|---|---:|---:|---:|
| Fixed serial | 67,07 ms | 75,18 ms | 111,05 ms |
| Fixed dos fases | 70,85 ms | 106,04 ms | 112,47 ms |
| Block/Arena | 54,11 ms | 64,07 ms | 100,83 ms |
| Go-inspired | 68,51 ms | 104,73 ms | 118,73 ms |

El descenso de Block/Arena con snapshots no se explica por mantener el
Mutex durante la lectura de procfs. La ruta de admisión suelta el lock para
leer el snapshot, pero [`budget::reserve`](../../../src/stack/block_experimental/budget.rs)
espera en el `Condvar` mientras `admission.is_refreshing()` es verdadero. En
en las 80 muestras de Block con snapshots, completó exactamente la cuota
mínima de operaciones por worker; los workers quedaron bloqueados
por la puerta de refresh durante buena parte de las cinco lecturas. El p99
mediano por muestra se mantuvo bajo, pero el máximo observado llegó a 344 ms
con un worker, 280 ms con dos, 276 ms con cuatro y 361 ms con ocho. Por tanto,
el p99 aislado oculta las pausas largas.

Esto identifica una política de admisión explícita durante el refresh, no
demuestra contención causada por ejecutar procfs dentro del Mutex. Es una
limitación importante del prototipo Block/Arena que debe investigarse antes de
usar sus cifras para comparar el throughput durante snapshots con los otros
motores.

Las latencias instrumentadas durante snapshots (mediana de p50/p95/p99 por
muestra; máximo real entre las diez muestras) fueron. La instrumentación
registra todos los ciclos de la cuota mínima y uno de cada ocho ciclos
adicionales; los percentiles se calculan sobre estas muestras de latencia, no
sobre el tiempo total de snapshots:

| Motor | Workers | p50 / p95 / p99 | Máximo observado |
|---|---:|---:|---:|
| Fixed serial | 1 | 3,2 / 5,6 / 6,7 µs | 256 µs |
| Fixed serial | 2 | 10,1 / 15,9 / 19,6 µs | 1,665 ms |
| Fixed serial | 4 | 28,2 / 54,7 / 86,7 µs | 2,986 ms |
| Fixed serial | 8 | 60,5 / 118,0 / 186,3 µs | 16,670 ms |
| Fixed dos fases | 1 | 3,4 / 5,8 / 7,0 µs | 184 µs |
| Fixed dos fases | 2 | 7,2 / 11,6 / 14,5 µs | 226 µs |
| Fixed dos fases | 4 | 22,2 / 28,6 / 35,1 µs | 3,594 ms |
| Fixed dos fases | 8 | 55,0 / 67,2 / 96,8 µs | 5,951 ms |
| Block/Arena | 1 | 2,3 / 5,5 / 6,9 µs | 343,754 ms |
| Block/Arena | 2 | 4,2 / 9,8 / 12,1 µs | 279,711 ms |
| Block/Arena | 4 | 11,1 / 18,2 / 24,3 µs | 276,326 ms |
| Block/Arena | 8 | 29,1 / 42,4 / 59,2 µs | 361,395 ms |
| Go-inspired | 1 | 0,2 / 0,2 / 0,2 µs | 208 µs |
| Go-inspired | 2 | 0,5 / 0,8 / 0,9 µs | 201 µs |
| Go-inspired | 4 | 1,3 / 2,1 / 2,5 µs | 202 µs |
| Go-inspired | 8 | 2,4 / 7,5 / 10,0 µs | 10,014 ms |

Para Go-inspired, los acumulados medianos de espera/posesión del lock del pool
en una muestra instrumentada con snapshots fueron 394/1.203 ns (1 worker),
1.252/2.377 ns (2), 9.244/4.830 ns (4) y 116.259/8.982 ns (8). No se
registraron `mmap`, `mprotect` ni `munmap` en esos ciclos de reutilización
calientes. Son acumulados por muestra, no latencias por operación ni una
medición de creación de spans.

## Conclusiones y límites

1. Go-inspired es el más rápido en reutilización local y reserva fría en estas
   condiciones. No reduce VMAs y retiene alrededor de 6,15 GB de espacio
   virtual hasta destruir el manager en la revisión medida por este informe.
   Esa ejecución es anterior a la API experimental de vaciado y recuperación
   de spans de [ADR 013](../../decisions/013-span-reclamation.md); no mide
   `trim()` ni permite concluir cuánto se recupera con esa ruta.
2. Fixed dos fases no mostró una mejora consistente frente al Fixed serial en
   reserva fría o throughput concurrente. Block/Arena sí mejora su ruta de
   reutilización frente a Fixed, pero su puerta de refresh provoca una pausa
   severa durante snapshots de alta cardinalidad.
3. Las cifras no son evidencia de scheduler, goroutines ejecutables, cambio de
   contexto ni Dynamic Growable Stack. La cache por worker de Go-inspired
   simula ownership local; no es un Processor real.
4. No se promueve ningún prototipo. Fixed Stack permanece como implementación
   vigente; F2 sigue abierta y F3 no se inicia por este resultado.

Los logs `20261114T185923Z-allocator-tournament-4022958.log` y
`20261114T190725Z-allocator-tournament-4047928.log` son iteraciones anteriores.
El primero usaba barreras por ronda que podían dejar a los workers esperando
entre snapshots; el segundo ejecutaba ciclos continuos, pero incluía la
preparación de caches en el cronómetro y submuestreaba también la cuota mínima.
El runner final excluye esa preparación y conserva todas las latencias de la
cuota antes de submuestrear. Este informe usa exclusivamente la última
ejecución.

Comando reproducible: `./scripts/bench-go-inspired-stack.sh`. La metodología
vigente está en [`methodology.md`](../methodology.md).
