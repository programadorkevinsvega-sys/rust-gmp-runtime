# F4.3 — Estabilización y costes de dispatch

## Entorno y procedimiento

- Commit medido: `a32c69a` (`Stabilize F4 dispatch semantics and measurements`).
- Comando: `RUSTUP_TOOLCHAIN=1.85.0 cargo bench --bench machine_dispatch_stabilized`.
- Toolchain: `rustc 1.85.0 (4d91de4e4 2025-02-17)`.
- Sistema: Linux x86_64, kernel `7.0.0-42-generic`, Ubuntu.
- CPU: 12th Gen Intel Core i5-12450H; governor observado: `powersave`.
- Workloads: no-op, CPU ligera (64 iteraciones) y moderada (2.048
  iteraciones). 30 batches medidos de 128 G, 64 G de calentamiento por
  combinación, workers 1/2/4 y orden rotativo. Stacks de 32 KiB.
- El cronómetro de submit incluye publicación, round-robin y envío al worker;
  completion drain incluye ejecución, finalización, notificación y joins. Son
  fases observadas del dispatch real, no componentes internamente instrumentados.
- Los microbenchmarks por componente se ejecutan por separado y no son sumandos
  del end-to-end. MAD es la desviación absoluta mediana; rango es
  máximo-mínimo. No se estima p99.

## Dispatch completo

Nanosegundos por batch de 128 rutinas:

| Workers | Carga | Mediana | MAD | p95 | Máximo observado | Rango |
|---:|---|---:|---:|---:|---:|---:|
| 1 | No-op | 675,917 | 38,381 | 825,483 | 1,216,361 | 603,617 |
| 1 | CPU ligera | 656,983 | 45,074 | 908,352 | 1,050,767 | 452,156 |
| 1 | CPU moderada | 660,376 | 36,186 | 892,631 | 1,164,020 | 560,599 |
| 2 | No-op | 559,990 | 45,184 | 849,119 | 870,412 | 375,871 |
| 2 | CPU ligera | 620,571 | 98,385 | 1,064,563 | 1,123,595 | 627,953 |
| 2 | CPU moderada | 620,559 | 85,107 | 936,943 | 1,061,947 | 588,931 |
| 4 | No-op | 811,167 | 96,742 | 1,045,714 | 1,083,682 | 444,209 |
| 4 | CPU ligera | 827,170 | 70,785 | 1,055,956 | 1,091,360 | 435,926 |
| 4 | CPU moderada | 838,964 | 61,702 | 1,030,148 | 1,037,771 | 390,500 |

Con esta máquina y batch corto, cuatro workers fueron más lentos que uno o dos
en las tres cargas. Dos workers mejoraron la mediana de no-op, pero la variación
de workloads CPU se solapa; no es evidencia de escalabilidad. La mayor parte
del end-to-end cae en completion drain, que combina ejecución, cambio de
contexto, ciclo terminal, entrega y espera; no permite atribuir todo ese tiempo
a un único componente.

## Microbenchmarks aislados

Nanosegundos por batch de 128 operaciones:

| Componente | Mediana | MAD | p95 | Máximo observado | Rango |
|---|---:|---:|---:|---:|---:|
| Publicar G | 4,297 | 230 | 6,358 | 6,680 | 2,715 |
| Round-robin atómico | 794 | 10 | 923 | 928 | 147 |
| Canal mpsc entre threads | 81,361 | 22,993 | 214,909 | 276,286 | 232,736 |
| Cola local P, enqueue/dequeue | 8,609 | 431 | 10,763 | 15,583 | 8,159 |
| Permiso, claim/yield | 10,800 | 404 | 14,556 | 15,068 | 4,819 |
| Ciclo terminal del permiso | 10,492 | 404 | 15,818 | 29,609 | 19,603 |
| `run_once` one-shot | 183,293 | 20,432 | 320,761 | 324,146 | 170,730 |

Estos números delimitan el trabajo para una investigación futura: round-robin
es pequeño en aislamiento; `mpsc` entre threads y `run_once` son componentes
visibles, mientras que la espera/join y la concurrencia del host tienen alta
variación. No se cambia ni elimina ninguna responsabilidad de F4/F5 ni se
propone una optimización en base a esta sola medición.

## Estabilización de corrección

El full gate pasó con 62 pruebas aprobadas y 2 manuales ignoradas. Una prueba
con productores concurrentes valida exactamente una ejecución por G para
128, 1.000 y 10.000 rutinas con 1, 2 y 4 Machines, estados `Running` durante
la entrada, finalización terminal, asociación/liberación P-M y shutdown.
Pruebas adicionales cubren panic, dispatch rechazado y destrucción de una tarea
pendiente al caer su worker; el handle de esa tarea devuelve un error y no
queda esperando indefinidamente.

El permiso de ejecución y el resultado sobreviven en el trampoline hasta que el
context switch vuelve al stack del worker. La transición a `Dead` ocurre después
del retorno; no se presenta una G como terminada mientras todavía se ejecuta en
su stack.

La salida completa, incluyendo las 30 muestras por configuración y cada
microbenchmark, está en [los datos brutos](./2026-11-14-f4-3-stabilization-raw.txt).
