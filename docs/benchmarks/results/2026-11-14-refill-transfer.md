# Transferencia entre cachés locales y pool global

## Alcance y entorno

Se ejecutó `scripts/bench-go-inspired-stack.sh` completo con Fixed Stack y sus
defaults de producción sin cambios. El experimento agregó comparación de
targets fijos/adaptativo, refills sincronizados y ciclos con vidas de stack
intercaladas. El host fue Linux 7.0.0-42-generic x86_64, Rust 1.85.0 y 12 CPUs
lógicos; el proceso comenzó con 45 VMAs y el límite fue 1.048.576. El preflight
permitió el torneo. Se conservaron diez muestras por combinación, 100.000
stacks retenidos y 1/2/4/8 workers. El [log bruto](./runs/20261114T205908Z-allocator-tournament-209238.log)
incluye el resto de motores, tamaños, escenarios, estados antes/después y
contabilidad.

Las cifras de throughput de abajo agregan operaciones y tiempo transcurrido de
las diez muestras; no son la mediana de diez throughputs. Los percentiles
agrupan latencias medidas de las diez muestras. En las fases de demanda, una
latencia corresponde a un epoch; en las olas, a una ola sincronizada; en vidas,
a una operación. Los máximos son sensibles a pausas excepcionales. El tiempo
de locks es una suma instrumentada, no una latencia individual.

## Demanda variable: 8 workers, caché de 32 KiB

El calendario fijo fue `[1, 4, 16, 16, 4, 1, 0, 0]`, repetido durante 256
epochs por muestra y worker. La capacidad del cache fue de dos slots; el target
base fue uno. La variante adaptativa subía el target después de dos refills
agotados consecutivos y lo reducía tras dos epochs de baja demanda u ociosidad.

Throughput agregado según concurrencia, siempre con caché de 32 KiB y sin
instrumentación:

| Workers | Fijo predeterminado (1) | Fijo a capacidad (2) | Adaptativo (1–2) |
|---:|---:|---:|---:|
| 1 | 3.198.156 | 3.038.642 | 3.283.280 |
| 2 | 1.923.586 | 2.262.334 | 2.246.047 |
| 4 | 1.703.030 | 1.965.036 | 1.990.794 |
| 8 | 1.398.579 | 1.648.832 | 1.656.597 |

El throughput cae al aumentar los workers en esta carga dominada por refills;
el target mayor mejora el resultado en 2/4/8 workers, pero no en uno. Esto no
demuestra que el pool lock sea la única causa. La medición instrumentada de
ocho workers registra espera/posesión acumuladas de 562,43/36,73 ms para target
1, 453,02/30,83 ms para target 2 y 465,91/31,65 ms para el adaptativo.

La sensibilidad al presupuesto de caché con ocho workers también fue
dependiente del caso:

| Caché | Target inicial / capacidad (slots) | Fijo inicial (ops/s; refills) | Fijo a capacidad (ops/s; refills) | Adaptativo (ops/s; refills) |
|---:|---:|---:|---:|---:|
| 32 KiB | 1 / 2 | 1.398.579; 87.040 | 1.648.832; 43.520 | 1.656.597; 46.080 |
| 64 KiB | 2 / 5 | 2.081.839; 35.840 | 2.011.850; 17.920 | 1.930.081; 20.480 |
| 128 KiB | 5 / 10 | 2.487.758; 15.360 | 2.804.721; 7.680 | 2.724.429; 10.240 |
| 256 KiB | 10 / 21 | 2.459.640; 5.120 | 2.570.446; 2.560 | 2.693.212; 5.120 |

Así, elevar el target o el presupuesto no mejora monótonamente el throughput;
no hay un tamaño universal ganador en esta matriz.

Percentiles y deltas detallados para la caché predeterminada y ocho workers:

| Target | Throughput (ops/s) | p50 epoch (ns) | p95 (ns) | p99 (ns) | Máximo (ns) | Cambios de target | Solicitudes / slots |
|---|---:|---:|---:|---:|---:|---:|---:|
| Fijo predeterminado (1) | 1.398.579 | 6.798 | 116.744 | 148.368 | 1.261.267 | 0 | 87.040 / 87.040 |
| Fijo capacidad (2) | 1.648.832 | 5.326 | 99.776 | 130.452 | 1.646.415 | 0 | 43.520 / 87.040 |
| Adaptativo (1–2) | 1.656.597 | 6.315 | 99.184 | 125.902 | 260.756 | 5.120 | 46.080 / 87.040 |

Hits/misses fueron 20.480/87.040 con target fijo 1, 64.000/43.520 con target
2 y 61.440/46.080 con el controlador. El adaptativo aumentó el throughput cerca
de 18,4 % respecto al default, pero solo cerca de 0,5 % respecto al target fijo
2; hubo 5.120 cambios de target en el escenario de ocho workers. Esa pequeña
diferencia y la actividad de cambios no justifican todavía una política
automática de runtime. La fase es un workload sintético por epochs, no una
carga del scheduler ni una simulación de presión de memoria.

## Refills sincronizados

Cada worker agotó su cache y esperó en una barrera antes de cada ola; se
ejecutaron 128 olas por muestra. Con ocho workers y target 1 se hicieron
10.240 operaciones, 10.240 solicitudes y se transfirieron exactamente 10.240
slots. El throughput no instrumentado fue 433.969 ops/s. La latencia por ola
fue p50 4.393 ns, p95 8.502 ns, p99 10.453 ns y máximo 53.400 ns. El pico
observado fue de ocho refills simultáneos sin instrumentación y siete con
instrumentación.

En la variante instrumentada el throughput fue 376.737 ops/s; la espera
acumulada del lock del pool fue 41,56 ms y la posesión acumulada 4,28 ms
durante la muestra combinada. La instrumentación cambia el coste medido y no
debe compararse directamente con el throughput no instrumentado. Los contadores
de fallos de asignación y limpieza fueron cero y la contabilidad volvió al
baseline retenido.

## Vidas intercaladas y retención de cache

Cada worker ejecutó 2.000 operaciones por muestra con el calendario de vida
`[1, 4, 16]`; se vaciaron las cachés cada 1, 8 o 64 operaciones. Con ocho
workers y caché de 32 KiB:

| Flush cada operaciones | Throughput (ops/s) | p50 (ns) | p95 (ns) | p99 (ns) | Máximo (ns) | Solicitudes / slots |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1.005.126 | 5.625 | 20.089 | 29.691 | 665.289 | 160.000 / 160.000 |
| 8 | 2.774.326 | 1.998 | 5.993 | 10.446 | 446.253 | 20.480 / 20.480 |
| 64 | 3.058.393 | 1.671 | 5.026 | 7.953 | 2.889.042 | 3.040 / 3.040 |

El intervalo de 64 operaciones multiplicó el throughput por 3,04 frente al
flush por operación y redujo las solicitudes unas 52,6 veces. Es evidencia de
que conservar slots locales amortiza las transferencias; no establece una
política universal de retención. Los máximos muestran pausas puntuales incluso
cuando los percentiles son menores. La instrumentación reflejó las mismas
solicitudes exactas; con flush cada operación acumuló 1.164,96 ms de espera y
75,83 ms de posesión de lock en diez muestras, mientras que con flush cada 64
fueron 2,43 ms y 3,47 ms, respectivamente.

## Conclusión

La transferencia por lotes reduce solicitudes cuando los slots permanecen en
la cache; tanto el refill sincronizado como el flush frecuente exponen el coste
del pool compartido. El target 2 reduce las solicitudes a la mitad con la misma
cantidad de slots transferidos que el target 1 en la fase de demanda, pero la
mejora entre fijo 2 y adaptativo es marginal. Por tanto, se mantiene el target
local bajo control explícito y la retención actual; no se habilita adaptación
automática, no se cambia el default de 32 KiB y no se promueve el allocator
Go-inspired a producción con esta evidencia. El protocolo no incluye Processor
real, quiescencia del scheduler, presión adaptativa de memoria ni snapshots VMA
concurrentes. Una siguiente decisión requiere muestras independientes
adicionales y una integración que represente ownership y vida de los P.
