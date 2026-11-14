# Comparación del allocator Fixed Stack: serial vs. reserva en dos fases

## Alcance y entorno

Se compararon dos rutas del mismo `StackManager` en Linux x86_64 con Rust
1.85.0, límite `vm.max_map_count=1.048.576` y 100.000 stacks retenidos
simultáneamente. Ambas usan Fixed Stack, guard pages, el mismo tamaño de stack,
preflight VMA habilitado con su margen predeterminado y la misma lógica de
reconciliación. La ruta `serialized` es el allocator original; `two_phase` es
el prototipo opt-in que ejecuta syscalls fuera del mutex de admisión.

Para cada estrategia se midieron 1, 2, 4 y 8 workers, diez repeticiones de
5.000 ciclos `allocate()`→`Drop` por worker y el snapshotter adicional apagado
y encendido. Con snapshotter apagado, el preflight siguió activo y pudo
refrescar por cadencia; apagado significa que no hubo un thread pidiendo
snapshots concurrentes. Con snapshotter encendido, un thread ejecutó snapshots
reales mientras trabajaban los workers.

La instrumentación por worker registra todas las adquisiciones, el tiempo total
con el mutex por ciclo completo y `mmap`, `mprotect`, `munmap` por syscall. Los
percentiles son de las latencias individuales de las diez muestras. Los
timers/atomics añaden overhead, por lo que estos resultados comparan los dos
caminos instrumentados y no equivalen a cifras de producción.

## Throughput y distribución de latencia

| Ruta | Snapshots externos | Workers | Throughput mediano (ops/s) | p50 (ns) | p95 (ns) | p99 (ns) | Máximo (ns) |
|---|---|---:|---:|---:|---:|---:|---:|
| Serial | Off | 1 | 284.573 | 2.833 | 6.216 | 8.528 | 159.098 |
| Dos fases | Off | 1 | 274.178 | 3.014 | 6.573 | 8.720 | 205.213 |
| Serial | Off | 2 | 206.925 | 7.424 | 16.938 | 22.585 | 2.075.396 |
| Dos fases | Off | 2 | 294.043 | 5.755 | 12.181 | 17.356 | 2.189.695 |
| Serial | Off | 4 | 179.233 | 20.364 | 45.592 | 84.418 | 9.014.938 |
| Dos fases | Off | 4 | 241.001 | 15.829 | 22.518 | 28.205 | 2.085.462 |
| Serial | Off | 8 | 163.914 | 49.603 | 100.952 | 167.637 | 2.978.329 |
| Dos fases | Off | 8 | 163.495 | 46.835 | 60.406 | 82.858 | 5.088.265 |
| Serial | On | 1 | 263.122 | 3.232 | 6.332 | 8.693 | 197.352 |
| Dos fases | On | 1 | 231.601 | 3.371 | 6.901 | 9.316 | 411.505 |
| Serial | On | 2 | 200.046 | 8.450 | 17.096 | 23.253 | 2.139.985 |
| Dos fases | On | 2 | 244.202 | 7.330 | 11.504 | 14.134 | 333.497 |
| Serial | On | 4 | 158.948 | 24.300 | 48.074 | 81.423 | 11.881.883 |
| Dos fases | On | 4 | 211.081 | 17.610 | 24.817 | 30.775 | 3.182.174 |
| Serial | On | 8 | 148.012 | 52.734 | 116.556 | 231.266 | 3.251.686 |
| Dos fases | On | 8 | 158.599 | 47.932 | 60.791 | 79.425 | 6.375.294 |

Con snapshots externos apagados, dos fases eleva el throughput mediano un
42,1 % con 2 workers y un 34,5 % con 4; con 8 queda prácticamente igual
(-0,3 %). Con snapshots encendidos, mejora 22,1 %, 32,8 % y 7,2 % con 2, 4 y
8 workers, respectivamente. Con un worker no gana throughput: el camino serial
resulta 3,7 % más rápido sin snapshotter y 12,1 % más rápido con él. El
prototipo reduce especialmente la cola: a 8 workers el p99 baja 50,6 % sin
snapshots externos y 65,7 % con ellos. El máximo observado aumenta en algunos
escenarios; no se interpreta como p99 ni como garantía.

## Espera, mutex y syscalls

Medianas de diez muestras de los promedios por worker, en nanosegundos:

| Ruta | Snapshots | Workers | Espera/adquisición | Mutex/ciclo completo | `mmap` | `mprotect` | `munmap` |
|---|---|---:|---:|---:|---:|---:|---:|
| Serial | Off | 1 | 22 | 3.154 | 1.004 | 796 | 1.108 |
| Dos fases | Off | 1 | 22 | 179 | 1.029 | 798 | 1.089 |
| Serial | Off | 2 | 2.162 | 4.038 | 983 | 1.016 | 1.339 |
| Dos fases | Off | 2 | 36 | 252 | 1.568 | 2.040 | 2.022 |
| Serial | Off | 4 | 8.079 | 5.162 | 1.299 | 1.298 | 1.666 |
| Dos fases | Off | 4 | 40 | 266 | 4.800 | 5.286 | 5.308 |
| Serial | Off | 8 | 20.995 | 5.832 | 1.472 | 1.396 | 1.992 |
| Dos fases | Off | 8 | 48 | 290 | 14.686 | 15.654 | 16.619 |
| Serial | On | 1 | 22 | 3.379 | 1.052 | 862 | 1.210 |
| Dos fases | On | 1 | 23 | 192 | 1.221 | 960 | 1.254 |
| Serial | On | 2 | 2.382 | 4.408 | 1.062 | 1.062 | 1.484 |
| Dos fases | On | 2 | 34 | 240 | 2.064 | 2.305 | 2.610 |
| Serial | On | 4 | 9.191 | 5.926 | 1.549 | 1.487 | 1.882 |
| Dos fases | On | 4 | 41 | 270 | 5.449 | 6.058 | 6.022 |
| Serial | On | 8 | 22.923 | 6.386 | 1.662 | 1.661 | 2.010 |
| Dos fases | On | 8 | 48 | 305 | 15.480 | 16.665 | 15.902 |

La columna “Mutex/ciclo completo” es la suma instrumentada de los tramos
protegidos del `allocate()`→`Drop`; “espera/adquisición” está normalizada por
adquisición. Dos fases hace más adquisiciones (incluidas las confirmaciones y
operaciones de preflight), por lo que no se comparan solo los totales de
adquisiciones. A 8 workers, la espera por adquisición cae de ~21–23 µs a ~48
ns, y la posesión acumulada por ciclo de ~5,8–6,4 µs a ~0,29–0,31 µs.

Al soltar el lock, aumenta el tiempo observado de syscalls bajo contención: a
8 workers cada syscall ronda 15–17 µs frente a 1,4–2,0 µs en la ruta serial.
El kernel ahora recibe más syscalls simultáneamente. Por tanto, el prototipo
reduce serialización y latencia de cola, pero no elimina el límite agregado del
kernel: el throughput a 8 workers no mejora sin snapshots externos, y mejora
solo modestamente con ellos. Las duraciones de syscall incluyen espera y
trabajo del kernel; no atribuyen causalidad a un componente específico.

## Snapshots, contabilidad y errores

El snapshotter tuvo snapshots de aproximadamente 54–81 ms por lectura según
la cardinalidad y la carga. Esos tiempos no se incluyen como posesión del mutex.
Las lecturas se ejecutaron fuera del lock en ambas rutas.

En cada una de las 160 muestras:

- `active_mappings_before == active_mappings_after == 100.000`.
- `reserved_bytes_before == reserved_bytes_after == 1.228.800.000`.
- Fallos de asignación, fallos de cleanup y rechazos del preflight fueron cero.
- Los snapshots vieron aproximadamente 200.000 VMAs. Después de liberar los
  100.000 stacks retenidos, ambas rutas terminaron con cero mappings activos y
  54 VMAs observadas.

La salida completa incluye contadores por worker y por muestra en
[el log bruto](./runs/20261114T134910Z-3234248.log). La suite completa
`scripts/bench-all.sh` terminó con código 0; el preflight también rechazó la
escala de 1M sin forzar reservas. Los caminos de cleanup fallido se probaron
separadamente: cleanup exitoso cancela el pending; cleanup `munmap` fallido
mantiene la capacidad reservada, registra el error y devuelve el error
compuesto. La política fatal de `munmap` en `Drop` pasó en proceso hijo tanto
para la ruta serial como para dos fases.

## Decisión y pendientes

El resultado justifica conservar el prototipo para pruebas y evaluación, no
promoverlo todavía como comportamiento predeterminado. Mejora throughput a 2–4
workers y reduce p95/p99 en las cargas medidas, pero no mejora throughput a 8
workers sin snapshots externos y eleva el coste de los syscalls concurrentes.
Se requieren repeticiones independientes, análisis de variabilidad y más
pruebas de refresh/rechazo durante estados pendientes antes de cambiar el
default. No se modificaron Fixed Stack, el scheduler, el context switching ni
la estrategia de slabs.

La decisión y el modelo de estados están registrados en
[ADR 009](../../decisions/009-two-phase-stack-reservations.md). La medición
anterior de alta cardinalidad sigue siendo una referencia independiente en
[su informe previo](./2026-11-14-allocator-high-cardinality.md).
