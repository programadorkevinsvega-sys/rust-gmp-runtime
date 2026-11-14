# Medición de batches y amortización de refill

## Alcance y entorno

Se ejecutó `scripts/bench-go-inspired-stack.sh` completo con el allocator
Go-inspired experimental, sin cambiar Fixed Stack ni los defaults. El host era
Linux 7.0.0-42-generic x86_64, Rust 1.85.0 y 12 CPUs lógicos. El proceso
comenzó con 45 VMAs y límite de 1.048.576; el preflight permitió la ejecución.
La salida completa está en
[20261114T203419Z-allocator-tournament-136274.log](./runs/20261114T203419Z-allocator-tournament-136274.log).

La matriz de refill retuvo 100.000 stacks, probó 1/2/4/8 workers, diez
muestras por combinación y presupuestos de caché de 32/64/128/256 KiB. Se
midieron caché caliente, refill forzado y ráfagas. El refill forzado vacía la
caché entre operaciones; las ráfagas usan 2× o 4× el target efectivo, mantienen
simultáneamente vivos los stacks de cada ráfaga y conservan la caché local entre
ráfagas dentro de cada muestra. El runner comienza una caché vacía en cada
muestra y excluye del tiempo de operaciones el flush final, cuyos timings se
registran aparte. Para las ráfagas se recopilaron 256 iteraciones por worker y
muestra.

Las cifras siguientes son medianas de diez muestras. El throughput es de
operaciones completas de reserva/liberación por segundo. Las latencias son
percentiles **por ráfaga**; los tamaños de ráfaga varían con el target, por lo
que no deben interpretarse como una comparación de latencia por stack entre
targets distintos. Los resultados sin instrumentación se usan para
throughput; los contadores de locks se recogen en la ejecución instrumentada.

## Throughput y amortización, 8 workers

| Caché | Target real | Refill forzado (ops/s) | Ráfaga 2× (ops/s) | Ops/refill 2× | Ráfaga 4× (ops/s) | Ops/refill 4× |
|---:|---:|---:|---:|---:|---:|---:|
| 32 KiB | 1 | 862.401 | 2.528.096 | 256,00 | 1.631.499 | 1,99 |
| 64 KiB | 2 | 765.185 | 2.587.244 | 512,00 | 2.015.972 | 3,98 |
| 128 KiB | 5 | 653.762 | 2.496.007 | 1.280,00 | 2.283.375 | 9,96 |
| 256 KiB | 10 | 617.295 | 2.632.289 | 2.560,00 | 2.062.964 | 19,92 |

En el refill forzado se hicieron 160.000 solicitudes por combinación de ocho
workers y diez muestras. Los slots transferidos fueron exactamente 160.000,
320.000, 800.000 y 1.600.000, respectivamente; el máximo observado por batch
coincidió con el target 1/2/5/10. Por tanto, el pool sí entrega el batch
configurado cuando hay slots disponibles. El batch de un slot del default de
32 KiB es consecuencia de su stride de 12 KiB, capacidad entera de dos slots y
target de la mitad, no de una ruta que esté ignorando batches mayores.

La prueba forzada penaliza cada target mayor: la mediana de throughput baja de
862 k ops/s con target 1 a 617 k ops/s con target 10. Es deliberado:
se solicita un refill por operación y se devuelve al pool el resto del batch,
de modo que esta prueba mide contención y coste de transferencia bajo estrés,
no el amortizado de una caché caliente.

En las ráfagas de 2× target, tras el llenado inicial la caché puede retener la
ráfaga completa; se observaron solo 160 refills en total para los ocho workers
y diez muestras. En las de 4× target el tamaño excede la capacidad local y se
mantiene la presión: el número de operaciones por refill se aproxima a 2× el
target. Esto demuestra la amortización cuando el trabajo reutiliza la caché,
pero también muestra que un burst mayor no equivale a más throughput de forma
monótona.

## Latencia por ráfaga

Percentiles medianos sobre las diez muestras no instrumentadas, en
nanosegundos:

| Target | Ráfaga | p50 | p95 | p99 | Máximo de muestra |
|---:|---:|---:|---:|---:|---:|
| 1 | 2× | 4.335 | 10.835 | 14.182 | 34.184 |
| 1 | 4× | 16.417 | 38.385 | 53.252 | 344.614 |
| 2 | 2× | 7.749 | 28.112 | 37.042 | 71.280 |
| 2 | 4× | 28.519 | 54.620 | 71.027 | 221.443 |
| 5 | 2× | 22.432 | 66.038 | 86.616 | 125.571 |
| 5 | 4× | 65.114 | 107.859 | 129.822 | 352.259 |
| 10 | 2× | 32.325 | 144.882 | 268.381 | 344.744 |
| 10 | 4× | 146.498 | 223.233 | 268.175 | 475.060 |

Cada percentil de la tabla es la mediana del percentil calculado dentro de cada
muestra (2.048 ráfagas por muestra para ocho workers). El máximo es la mediana
de los máximos de muestra, no el mayor valor global. Como cada ráfaga contiene
más stacks al crecer el target, estas cifras no atribuyen por sí solas el
aumento a una regresión por stack; haría falta una prueba adicional con
cardinalidad fija de ráfaga y medición de latencia individual.

## Contención y decisión

En la variante instrumentada de ocho workers, las ráfagas 4× acumularon,
durante diez muestras, espera/posesión del lock del pool de 304/21,3 ms
(target 1), 530/39,9 ms (target 2), 1.034/80,1 ms (target 5) y
1.935/148,8 ms (target 10). El refill forzado acumuló 1,25/79,5 ms,
1,36/94,0 ms, 1,57/110,8 ms y 1,80/124,0 ms, respectivamente. Son totales
instrumentados, no latencias por operación. No hubo fallos de asignación ni
de limpieza en estos escenarios.

La evidencia confirma batches efectivos y amortización en la ruta caliente,
pero no justifica aumentar el presupuesto predeterminado: el escenario forzado
empeora y el de ráfagas no muestra un target ganador consistente; a mayor
target crecen el batch y la contención acumulada, y cambian la duración y la
cardinalidad de las ráfagas. El experimento tampoco representa todavía la
carga del scheduler G-M-P. Se mantiene el default de 32 KiB y la coordinación
explícita de cachés. No se implementan slabs ni una política automática de
retención.
