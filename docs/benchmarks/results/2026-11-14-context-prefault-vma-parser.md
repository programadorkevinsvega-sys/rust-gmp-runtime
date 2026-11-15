# Contexto pre-tocado y parser VMA

## Entorno y comando

- Rust: `rustc 1.85.0 (4d91de4e4 2025-02-17)`.
- Host: Intel Core i5-12450H, 12 CPUs lógicas, Linux 7.0.0-42-generic.
- Governor observado: `powersave`.
- Comandos:
  - `RUST_GMP_CONTEXT_BENCH_ITERATIONS=256 cargo +1.85.0 bench --bench context_switching`
  - `cargo +1.85.0 test --release --lib interleaved_context_switch_backend_comparison -- --ignored --nocapture`
  - `cargo +1.85.0 test --release --lib high_vma_parser_comparison -- --ignored --nocapture`

El código medido estaba sin commit. Los resultados caracterizan esta máquina y
no establecen una política de scheduling ni una mejora universal.

## Comparación A/B del switch

Se ejecutó `interleaved_context_switch_backend_comparison`: 20 muestras con
1.000.000 roundtrips por backend y muestra, alternando el orden entre muestras.
Ambos stacks de 32 KiB se pre-tocaron antes de medir. La cifra se divide entre
dos transferencias por roundtrip y se conserva con tres decimales. Se comparó
el switch antiguo (`jmp` a continuación y contexto de 80 bytes) con el actual
(`push`/`pop` y contexto de 16 bytes):

| Muestra | Anterior ns/transferencia | Actual ns/transferencia | Actual - anterior |
|---:|---:|---:|---:|
| 0 | 11,495 | 10,344 | -1,151 |
| 1 | 11,241 | 10,336 | -0,905 |
| 2 | 11,483 | 10,279 | -1,204 |
| 3 | 11,092 | 10,167 | -0,925 |
| 4 | 11,179 | 10,358 | -0,821 |
| 5 | 12,066 | 10,746 | -1,320 |
| 6 | 11,487 | 10,184 | -1,304 |
| 7 | 11,778 | 10,168 | -1,610 |
| 8 | 12,100 | 10,736 | -1,364 |
| 9 | 11,176 | 10,270 | -0,906 |
| 10 | 11,327 | 10,194 | -1,133 |
| 11 | 11,128 | 10,218 | -0,910 |
| 12 | 11,106 | 10,244 | -0,862 |
| 13 | 11,442 | 10,316 | -1,126 |
| 14 | 11,270 | 10,291 | -0,979 |
| 15 | 11,166 | 10,458 | -0,708 |
| 16 | 11,325 | 10,342 | -0,983 |
| 17 | 11,291 | 10,384 | -0,907 |
| 18 | 11,214 | 10,270 | -0,944 |
| 19 | 11,168 | 10,256 | -0,912 |

| Backend | p50 ns/transferencia | Rango |
|---|---:|---:|
| Anterior | 11,280 | 11,092–12,100 |
| Actual | 10,285 | 10,167–10,746 |
| Diferencia emparejada (actual - anterior) | -0,961 | -1,610–-0,708 |

Las 20 diferencias emparejadas de esta corrida favorecen al switch actual; su
mediana es -0,961 ns por transferencia (aproximadamente 8,5 % de la mediana del
anterior). Esto aporta evidencia local de una mejora pequeña en el loop aislado,
que no era visible con la resolución entera anterior. No es una estimación
universal ni una prueba formal de significación: es una corrida en un host y no
incluye intervalo de confianza ni replicación en sesiones independientes. El
impacto en `run_once` completo es mucho menor porque solo evita cerca de 2 ns
por rutina con dos transferencias, frente a los aproximadamente 157 ns del
caso pre-tocado. La medición no incluye page faults ni el overhead restante de
`run_once`.

## `run_once` frío y pre-tocado

Diez muestras de 256 rutinas cada una, stack de 32 KiB. Las rutinas se reservan
y publican fuera del cronómetro. Para el segundo caso, el benchmark escribe en
una dirección de cada página usable antes de medir. Ambos valores incluyen
trampoline, `catch_unwind`, lifecycle y dos transferencias; no son el coste de
un switch aislado.

| Muestra | Frío ns/rutina | Pre-tocado ns/rutina |
|---:|---:|---:|
| 0 | 2530 | 230 |
| 1 | 1338 | 156 |
| 2 | 1284 | 156 |
| 3 | 1408 | 166 |
| 4 | 1394 | 168 |
| 5 | 1296 | 147 |
| 6 | 1337 | 180 |
| 7 | 1388 | 154 |
| 8 | 1318 | 157 |
| 9 | 1249 | 261 |

Llamada directa (ns): `6, 6, 6, 6, 6, 6, 6, 6, 6, 6`.

| Caso | p50 ns/rutina | p95 ns/rutina | Mínimo | Máximo |
|---|---:|---:|---:|---:|
| Directo | 6 | 6 | 6 | 6 |
| Frío | 1337 | 2530 | 1249 | 2530 |
| Pre-tocado | 157 | 261 | 147 | 261 |

La diferencia de medianas frío/pre-tocado fue 1.180 ns por rutina en esta
ejecución. Es evidencia de que el primer toque de páginas afecta materialmente
al `run_once` compuesto; no atribuye todo el residual al context switch. En
frío, la mediana actual de 1.337 ns frente a los 1.210 ns de la línea base F3.3
no permite declarar regresión ni mejora sin una comparación A/B bajo las mismas
condiciones.

## Parser de `/proc/self/maps`

Se generaron 800.000 entradas ASCII sintéticas idénticas. Cada muestra compara
el parser actual byte-oriented con la validación previa basada en
`split_whitespace` y `from_str_radix`; no crea mappings. En las diez muestras,
ambos contaron las 800.000 entradas:

| Medida | Parser actual | Referencia anterior |
|---|---:|---:|
| Mediana | 57,47 ms | 80,58 ms |
| Rango | 54,72–59,53 ms | 79,76–82,70 ms |
| Reducción de mediana | 28,7 % | — |

Muestras (ms), ordenadas por ronda:

| Ronda | Actual | Referencia |
|---:|---:|---:|
| 0 | 54,718 | 81,625 |
| 1 | 55,934 | 82,701 |
| 2 | 57,786 | 80,335 |
| 3 | 57,603 | 79,862 |
| 4 | 58,057 | 79,762 |
| 5 | 56,851 | 80,895 |
| 6 | 59,529 | 81,180 |
| 7 | 56,886 | 80,503 |
| 8 | 57,338 | 80,171 |
| 9 | 57,052 | 80,666 |

La comparación sintética mide validación/conteo, no una lectura de procfs de
800.000 VMA. En ese workload sintético el parser actual cuesta cerca de 72 ns
por línea; no se afirma que sea óptimo. El test real
`vma_preflight_component_costs` también pasó con el preflight activo: en el
host, leer y parsear el snapshot actual tomó 34,85 µs por operación y el
snapshot completo 35,11 µs; el allocator con y sin preflight midió 2.118 y
2.050 ns por operación respectivamente en esa ejecución. No se debilitó la
admisión. El buffer se limita a 64 KiB porque la primera versión de 256 KiB
podía aparecer temporalmente como una VMA adicional y sesgar la propia
observación.

### Qué mide el costo de allocator con preflight

Los valores de allocator provienen de `report_allocator_modes`: una sola
muestra de 10.000 ciclos secuenciales `allocate()` + `drop()` por configuración,
con un `Instant` rodeando el lote completo. No se promedian diez lotes; el
`10_000` es el número de ciclos medidos. Los managers son nuevos y no tienen un
snapshot inicial.

| Pregunta | Respuesta para este lote |
|---|---|
| Asignaciones entre snapshots | Las 10.000 asignaciones caben en un único intervalo de snapshot. La cadencia configurada es cada 262.144 operaciones contables; en este workload un ciclo asignar/liberar suma dos, así que el refresco periódico ocurre tras 131.072 ciclos completos. |
| Lecturas reales de `/proc/self/maps` dentro del lote | Una: la primera asignación obtiene el snapshot inicial. Las 9.999 asignaciones restantes reutilizan ese snapshot. |
| ¿El tiempo de snapshot está dentro del cronómetro del allocator? | Sí. La lectura inicial sucede sincrónicamente dentro de la primera `allocate()` y queda incluida en el lote con preflight. |
| Reutilización e invalidación | El snapshot se comparte mediante el estado de admisión del manager y sus clones. Se refresca si no existe, al vencer la cadencia, ante un rechazo de capacidad que exige reconciliación, o mediante consultas explícitas `vma_snapshot()`/`metrics()` que solicitan observación fresca. |

Cada `allocate()` vuelve a comprobar el presupuesto contra el snapshot cacheado
y el accounting monotónico de reservas; por tanto, “reutilizar snapshot” no
significa omitir la comprobación de admisión. Una lectura de snapshot por lote
representa aproximadamente 3,5 ns amortizados por ciclo si se usa la medición
de 35,11 µs; la diferencia observada de 68 ns por operación entre los dos
lotes no se puede atribuir solo a esa lectura. Incluye el trabajo de lock y
comprobación cacheada, además del ruido de una muestra única por configuración.
El experimento separa el costo de snapshot del lote real, pero no instrumenta
un contador de lecturas dentro de esa ejecución; el conteo de una lectura se
deduce de la secuencia del test y de la cadencia, no de un contador observado.

Los 15,45 µs y la diferencia de 46 ns citados en una ejecución anterior
corresponden a otra corrida y no deben mezclarse con los valores de 34,85/35,11
µs y 68 ns de esta. En ambas, snapshot y allocator se midieron en intervalos
distintos; no es una comparación de “snapshot completo por asignación”.

## Cambios y límites

- El backend x86_64 usa `push`/`pop` para los seis registros callee-saved,
  manteniendo `cld`, el control word x87 y los bits de control MXCSR. El estado
  persistente de `MachineContext` pasó de 80 a 16 bytes. La prueba A/B no detectó
  una mejora de velocidad fuera de la dispersión.
- `RoutineContext` conserva los bounds inmutables del stack. `run_once` los
  consulta bajo el lock de contexto que ya necesita y evita tomar el mutex del
  stack para obtener esos bounds.
- El lector VMA procesa procfs con un buffer reutilizable de 64 KiB, mantiene
  errores explícitos y valida todos los campos estructurales antes de contar.
  Se añadieron tests para direcciones, permisos, offset, device, inode y línea
  malformada.
- El stack pre-tocado se accede mediante escrituras volátiles en un benchmark
  específico, sin cambiar el comportamiento de producción.

Los benchmarks son microbenchmarks locales y usan worktrees sin commit. No
demuestran escalabilidad del scheduler, el coste puro de Assembly ni el
comportamiento con una carga de aplicación real.
