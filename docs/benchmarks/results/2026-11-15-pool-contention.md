# Diagnóstico aislado del pool Go-inspired

## Alcance y método

`cargo +1.85.0 bench --bench pool_contention` compara el pool global con
cachés explícitas propiedad de cada worker. La matriz usa 1, 2, 4, 8, 12 y 16
workers, 1.024 ciclos de adquisición/devolución por worker y diez muestras por
configuración. Cada muestra corre en un proceso hijo nuevo; global y caché
local nunca están activos a la vez. El orden se alterna entre muestras para
reducir sesgo sistemático. Se usan stacks solicitados de 64 KiB, spans de 64
slots y un target de caché de 1 MiB.

Esta es una carga aislada del allocator: no ejecuta closures, `Routine`,
`MachineManager`, dispatch, join ni stealing. La ruta worker-cache es un
contraste experimental con handles de caché explícitos; no está conectada a
Processors del scheduler y no representa una integración de producción.
Cada operación global es `allocate()` seguido de `drop`; cada operación de
caché es `allocate_cached()` seguido de `release_to_cache()`.

Entorno: Rust 1.85.0, Linux x86_64, kernel `7.0.0-42-generic`, Intel Core
i5-12450H, 12 CPUs lógicas y governor `powersave`. El proceso no fija afinidad
ni elimina cargas externas del host.

## Throughput sin instrumentación

La corrida sin `allocator-benchmark` es la referencia de rendimiento. Se
muestran mediana y rango observado en diez procesos por celda; con diez
muestras no se estima un p99 robusto.

| Workers | Pool global (ops/s, mediana [rango]) | Caché por worker (ops/s, mediana [rango]) | Mejora caché/global |
|---:|---:|---:|---:|
| 1 | 4.204.961 [3.289.717–4.920.711] | 5.387.247 [4.875.703–6.156.325] | 1,28x |
| 2 | 2.208.346 [1.840.123–2.508.580] | 3.679.614 [3.064.492–4.989.086] | 1,67x |
| 4 | 2.051.686 [1.786.815–2.147.124] | 3.071.717 [2.933.761–3.222.834] | 1,50x |
| 8 | 1.571.622 [1.485.260–1.623.830] | 3.641.117 [3.373.120–3.894.782] | 2,32x |
| 12 | 1.481.952 [1.421.100–1.543.574] | 3.440.872 [3.286.619–3.870.534] | 2,32x |
| 16 | 1.485.048 [1.405.696–1.517.451] | 3.707.678 [3.602.718–3.935.899] | 2,50x |

Al pasar de 1 a 16 workers, el throughput global cae aproximadamente 65%;
el de caché local cae aproximadamente 31%. La separación entre rutas aumenta
con la concurrencia y alcanza 2,50x a 16 workers. Esto demuestra una penalidad
importante del camino global bajo este patrón de operaciones; por sí solo no
identifica de forma exclusiva el mutex, ya que las rutas difieren también en
la estructura de retorno y propiedad de slots.

## Diagnóstico instrumentado

Se repitió la misma matriz con
`cargo +1.85.0 bench --features allocator-benchmark --bench pool_contention`.
Las medidas de adquisición/devolución y lock son acumuladas por worker y se
dividen por el total de operaciones. No son tiempo de pared ni tiempo de un hilo
individual. La instrumentación añade `Instant` y contadores de diagnóstico, por
lo que estas cifras son diagnósticas, no comparables con el throughput sin
instrumentar.

| Workers | Espera agregada del mutex global (ns/op) | Mutex retenido (ns/op) | Adquisición global (ns/op) | Devolución global (ns/op) | Adquisición caché local (ns/op) | Devolución caché local (ns/op) |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 54 | 101 | 225 | 221 | 100 | 81 |
| 2 | 400 | 179 | 595 | 453 | 199 | 135 |
| 4 | 1.600 | 186 | 1.452 | 1.249 | 533 | 397 |
| 8 | 4.603 | 224 | 3.551 | 3.309 | 853 | 612 |
| 12 | 7.875 | 235 | 5.745 | 5.358 | 1.199 | 862 |
| 16 | 11.095 | 246 | 8.076 | 7.347 | 1.172 | 815 |

La espera acumulada por operación del pool global crece unas 205 veces entre
uno y dieciséis workers, mientras el tiempo acumulado dentro del mutex crece
mucho menos. En el camino de caché local, la espera medida del mutex del pool
es cero durante el tramo caliente. Junto con la diferencia de throughput sin
instrumentación, esto es evidencia sólida de que la contención del pool global
es un coste importante —y probablemente dominante dentro de este microbenchmark
de acquire/return— al aumentar los workers.

No demuestra que el mutex sea el cuello de botella dominante del scheduler ni
que explique por sí solo la regresión end-to-end de `Routine`. Ambos caminos
siguen actualizando contadores atómicos compartidos; por tanto, el descenso de
throughput que también persiste con cachés locales podría incluir contención o
tráfico de coherencia de esos contadores, planificación del SO y efectos de
caché/CPU. El benchmark tampoco compara rutas de publicación, stealing o
ejecución de trabajo útil.

## Caché, memoria y contabilidad

Las cachés se preparan antes de cronometrar. Para 1, 2, 4, 8, 12 y 16 workers,
respectivamente, la preparación registra 3, 6, 12, 24, 36 y 48 misses/refills,
con 21, 42, 82, 163, 241 y 323 slots transferidos. En el tramo caliente cada
operación de caché local fue un hit; no hubo misses ni refills. El pool global
no produjo actividad de caché.

Todas las 120 muestras por corrida completaron exactamente las operaciones
esperadas, sin fallos de asignación ni limpieza. La instrumentación confirmó
leases/releases balanceados, cero stacks activos al quiescer, geometría
consistente de spans, trim completo a cero y recuperación de memoria virtual
reservada/accesible. Esto comprueba contabilidad y lifecycle bajo esta carga;
no sustituye pruebas de memoria bajo stress prolongado ni una auditoría de
seguridad de todas las rutas del allocator.

La suite también pasó la prueba de no solapamiento de leases vivos del pool
global añadida en esta revisión y la prueba existente que provoca un fallo de
guard page en un proceso hijo. El crate declara `unsafe_code = "deny"`. Estas
comprobaciones reducen riesgos en las rutas probadas, pero no equivalen a una
auditoría formal de seguridad ni validan todos los backends de plataforma.

Datos brutos:

- [Corrida sin instrumentación](./runs/20261115-pool-contention-paired-uninstrumented.log)
- [Corrida instrumentada](./runs/20261115-pool-contention-paired-instrumented.log)
- [Código del benchmark](../../../benches/pool_contention.rs)

## Conclusión y siguiente experimento

La penalidad del mutex global sí está demostrada en el camino aislado de
adquisición/devolución; no está demostrada como causa dominante de la
degradación de `Routine` ni del scheduler. No se debe integrar todavía la caché
experimental ni rediseñar el pool basándose solo en este A/B.

El paso causal es ejecutar el mismo benchmark stackful de `Routine` con una
ruta de cache por Processor real, preservando la identidad y propiedad del
lease durante todo su lifecycle. Comparar ambos caminos en procesos
alternados, sin instrumentación para throughput, y usar una corrida diagnóstica
separada para adquisición, retorno y métricas de contención. Mantener iguales
el workload, número de workers, publicación, join y política de memoria; validar
exactly-once, exclusión de slots vivos y trim en ambos caminos.
