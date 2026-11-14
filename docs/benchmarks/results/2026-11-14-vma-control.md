# Diagnóstico VMA y preflight de stacks

## Condiciones

- Fecha: 2026-11-14. Árbol de trabajo basado en `fb240de`; el runtime y este
  informe estaban sin commit al medir.
- Linux 7.0.0-42-generic; Intel Core i5-12450H, 12 CPUs lógicas, governor
  `powersave`; Rust 1.85.0.
- `vm.max_map_count`: 1.048.576. VMA inicial y final observadas: 32.
- Comando: `RUST_GMP_RUN_SCALE=1 cargo +1.85.0 bench --bench scale`.
- Se calentó con 64 stacks y se ejecutaron 10 muestras por escala. Se midieron
  por separado el lote de reservas y el de liberaciones. El RSS es una lectura
  aproximada de `/proc/self/statm`; no equivale a memoria virtual ni a memoria
  comprometida.
- Se solicitaron 8 KiB usables por stack. Con páginas de 4 KiB, cada mapping
  reservado ocupó 12.288 bytes virtuales: una página de guardia y dos páginas
  usables. No se ejecutan rutinas sobre estos stacks en este benchmark.

## Resultados agregados

Los tiempos son nanosegundos; «dispersión» es máximo observado menos mínimo,
no un p99. La asignación y liberación no incluyen el calentamiento.

| Stacks | Reserva: mediana | Reserva: máximo | Reserva: dispersión | Liberación: mediana | Liberación: máximo | Liberación: dispersión |
|---:|---:|---:|---:|---:|---:|---:|
| 1.000 | 1.800.604 | 2.107.693 | 399.790 | 1.017.589 | 1.183.953 | 274.878 |
| 10.000 | 19.801.312 | 21.617.323 | 3.436.898 | 14.266.621 | 15.036.617 | 1.881.158 |
| 100.000 | 233.968.495 | 265.049.884 | 50.691.238 | 133.630.314 | 149.954.244 | 38.178.340 |
| 500.000 | 2.080.124.946 | 2.407.359.007 | 502.842.587 | 689.474.281 | 975.149.546 | 312.312.859 |

| Stacks activos | VMA antes / activos / después | Mappings runtime activos | Estimación runtime | Virtual reservado | Usable | RSS activo observado |
|---:|---:|---:|---:|---:|---:|---:|
| 1.000 | 32 / 2.031 / 32 | 1.000 | 2.000 | 12.288.000 B | 8.192.000 B | 2.478.080–2.486.272 B |
| 10.000 | 32 / 20.031 / 32 | 10.000 | 20.000 | 122.880.000 B | 81.920.000 B | 2.846.720–2.887.680 B |
| 100.000 | 32 / 200.031 / 32 | 100.000 | 200.000 | 1.228.800.000 B | 819.200.000 B | 6.447.104–6.848.512 B |
| 500.000 | 32 / 1.000.031 / 32 | 500.000 | 1.000.000 | 6.144.000.000 B | 4.096.000.000 B | 22.446.080–26.447.872 B |

Las VMA son el conteo real de líneas validadas en `maps`. «Mappings runtime»
cuenta los objetos de mapping activos del manager; «estimación runtime» aplica
la cota de hasta dos VMA por mapping. No son conteos exactos de VMA atribuibles
al runtime: Linux puede fusionar o dividir mappings. Para las cuatro escalas,
el benchmark registró cero rechazos y cero fallos nativos en cada muestra; tras
liberar, el conteo observado volvió a 32 y los bytes virtuales reservados por
el manager volvieron a cero.

### Datos brutos

Cada fila es una muestra. RSS se expresa en bytes como
`antes / activo / después`; los contadores de VMA y de mappings de cada escala
son los indicados en la tabla anterior. Las diez muestras de cada escala
registraron `rejections=0` y `failures=0`.

| Stacks | Muestra | Reserva (ns) | Liberación (ns) | RSS antes / activo / después (B) |
|---:|---:|---:|---:|---:|
| 1.000 | 0 | 2.107.693 | 1.183.953 | 2.371.584 / 2.478.080 / 2.478.080 |
| 1.000 | 1 | 1.997.208 | 1.029.267 | 2.478.080 / 2.478.080 / 2.478.080 |
| 1.000 | 2 | 1.756.758 | 992.144 | 2.478.080 / 2.478.080 / 2.478.080 |
| 1.000 | 3 | 1.810.537 | 1.005.912 | 2.478.080 / 2.478.080 / 2.478.080 |
| 1.000 | 4 | 1.964.659 | 1.143.673 | 2.478.080 / 2.478.080 / 2.478.080 |
| 1.000 | 5 | 1.733.917 | 1.052.512 | 2.478.080 / 2.478.080 / 2.478.080 |
| 1.000 | 6 | 1.737.265 | 909.075 | 2.478.080 / 2.478.080 / 2.478.080 |
| 1.000 | 7 | 1.790.671 | 1.004.290 | 2.478.080 / 2.478.080 / 2.478.080 |
| 1.000 | 8 | 1.838.134 | 946.253 | 2.478.080 / 2.486.272 / 2.486.272 |
| 1.000 | 9 | 1.707.903 | 1.176.113 | 2.486.272 / 2.486.272 / 2.486.272 |
| 10.000 | 0 | 18.180.425 | 14.143.762 | 2.486.272 / 2.887.680 / 2.486.272 |
| 10.000 | 1 | 19.844.533 | 14.359.984 | 2.486.272 / 2.846.720 / 2.846.720 |
| 10.000 | 2 | 19.529.605 | 14.651.022 | 2.846.720 / 2.846.720 / 2.846.720 |
| 10.000 | 3 | 20.566.930 | 13.643.731 | 2.846.720 / 2.846.720 / 2.846.720 |
| 10.000 | 4 | 19.361.248 | 13.155.459 | 2.846.720 / 2.846.720 / 2.846.720 |
| 10.000 | 5 | 19.903.416 | 14.457.851 | 2.846.720 / 2.846.720 / 2.846.720 |
| 10.000 | 6 | 20.052.472 | 14.173.258 | 2.846.720 / 2.846.720 / 2.846.720 |
| 10.000 | 7 | 19.758.091 | 13.945.800 | 2.846.720 / 2.846.720 / 2.846.720 |
| 10.000 | 8 | 19.654.926 | 14.379.238 | 2.846.720 / 2.846.720 / 2.846.720 |
| 10.000 | 9 | 21.617.323 | 15.036.617 | 2.846.720 / 2.846.720 / 2.846.720 |
| 100.000 | 0 | 241.033.212 | 111.775.904 | 2.846.720 / 6.848.512 / 2.846.720 |
| 100.000 | 1 | 236.475.304 | 149.954.244 | 2.846.720 / 6.447.104 / 6.447.104 |
| 100.000 | 2 | 238.069.392 | 140.204.944 | 6.447.104 / 6.447.104 / 6.447.104 |
| 100.000 | 3 | 265.049.884 | 124.942.091 | 6.447.104 / 6.447.104 / 6.447.104 |
| 100.000 | 4 | 214.358.646 | 134.629.756 | 6.447.104 / 6.447.104 / 6.447.104 |
| 100.000 | 5 | 248.038.989 | 140.437.284 | 6.447.104 / 6.447.104 / 6.447.104 |
| 100.000 | 6 | 231.461.687 | 130.959.849 | 6.447.104 / 6.447.104 / 6.447.104 |
| 100.000 | 7 | 229.527.678 | 130.255.635 | 6.447.104 / 6.447.104 / 6.447.104 |
| 100.000 | 8 | 220.826.056 | 139.910.273 | 6.447.104 / 6.447.104 / 6.447.104 |
| 100.000 | 9 | 229.731.380 | 132.630.873 | 6.447.104 / 6.447.104 / 6.447.104 |
| 500.000 | 0 | 2.317.947.934 | 662.836.687 | 6.447.104 / 26.447.872 / 6.447.104 |
| 500.000 | 1 | 2.407.359.007 | 683.748.369 | 6.447.104 / 22.446.080 / 22.446.080 |
| 500.000 | 2 | 2.031.096.955 | 875.015.487 | 22.446.080 / 22.446.080 / 22.446.080 |
| 500.000 | 3 | 2.115.180.640 | 691.632.548 | 22.446.080 / 22.446.080 / 22.446.080 |
| 500.000 | 4 | 2.125.764.439 | 671.350.503 | 22.446.080 / 22.446.080 / 22.446.080 |
| 500.000 | 5 | 1.904.516.420 | 682.389.470 | 22.446.080 / 22.446.080 / 22.446.080 |
| 500.000 | 6 | 1.914.737.396 | 687.316.014 | 22.446.080 / 22.446.080 / 22.446.080 |
| 500.000 | 7 | 1.993.476.269 | 975.149.546 | 22.446.080 / 22.446.080 / 22.446.080 |
| 500.000 | 8 | 2.045.069.253 | 752.742.661 | 22.446.080 / 22.446.080 / 22.446.080 |
| 500.000 | 9 | 2.226.374.243 | 812.926.371 | 22.446.080 / 22.446.080 / 22.446.080 |

## Escala de 1.000.000 y comparación F2

La escala de 1.000.000 se omitió antes de asignar: la estimación de 2.000.000
VMA supera el límite observado de 1.048.576, incluso antes de reservar el
margen configurable de 1.024. El preflight produjo un rechazo; no hubo fallo
nativo ni se intentó forzar la reserva.

El baseline F2 original se conserva en
[`informe original de F2`](2026-11-14-d9617743aba79bbdf62b3ea63c2a053cce5ef01c.md).
Sus medianas de reserva fueron 1.631.197, 18.980.719, 197.808.725 y
922.986.636 ns para 1k, 10k, 100k y 500k. Las mediciones actuales difieren
respectivamente en +10,39%, +4,32%, +18,28% y +125,37%. Es una comparación
orientativa, no una comparación pareada: el benchmark actual añade el coste de
diagnóstico/admisión, usa calentamiento explícito y separa liberación. La cifra
de 100k del baseline usa la mediana corregida de 197.808.725 ns, calculada a
partir de las dos muestras centrales 197.394.580 y 198.222.870 ns; su coste
mediano correcto es 1.978 ns/stack, no 1.965 ns/stack.

## Decisión y límites

No se cambió la estrategia de mappings individuales ni se adoptaron slabs. El
preflight reduce el riesgo de agotar el límite, pero no reserva capacidad del
kernel: otros componentes pueden mapear memoria concurrentemente y una llamada
`mmap` o `mprotect` aún puede fallar. Los contadores del runtime son
contabilidad propia y una estimación; solo `vma_current` es la observación
real del proceso. Estos resultados no afirman que una cantidad equivalente de
rutinas esté ejecutándose y no cierran por sí solos F2.

## Criterios de aceptación y pendientes

| Criterio | Estado | Evidencia o límite |
|---|---|---|
| Leer límite y VMA reales desde procfs | Cumplido | Pruebas Linux cotejan el snapshot con `/proc/sys/vm/max_map_count` y `/proc/self/maps`. |
| Integrar preflight con Stack Manager y margen configurable | Cumplido | Habilitado por defecto; falla cerrado si procfs no se puede consultar. |
| Controlar concurrencia y distinguir observación de estimación | Cumplido | Mutex compartido, prueba sintética concurrente y métricas separadas de VMA real, mappings propios y cota. |
| Gestionar fallos de reserva/protección y liberar mappings parciales | Cumplido en la estrategia existente | La prueba de fallo real de `mmap` y las pruebas de `mprotect`/`munmap` cubren errores; `Drop` conserva la política fatal si falla la liberación. |
| Métricas requeridas | Cumplido | Métricas de VMA, mappings/estimación, stacks, bytes, fallos y rechazos; el pool no existe y `free_stacks` es cero. |
| Unitarias, integración y concurrencia | Cumplido para F2 | `./scripts/check.sh`: 39 pruebas ejecutadas sin fallos, incluida la ruta fatal de `munmap` durante `Drop`, Clippy, formato y rustdoc satisfactorios. |
| Escalas 1k–500k y 1M condicionado a presupuesto | Cumplido con límite | 1k–500k medidas; 1M rechazado preventivamente sin forzar recursos. |
| Cero crates externos y Rust 1.85.0 | Cumplido | `cargo tree -e all` muestra solo el paquete runtime; compilación con 1.85.0. |
| Overflow al ejecutar sobre el stack de una rutina | Pendiente de F3 | No hay ejecución de código ni context switching sobre los stacks de rutina en F2. |
| Ciclo de vida completo de slabs | No implementado | Se conserva la estrategia individual; una alternativa requiere decisión y mediciones de allocator completas. |

El límite de VMA no garantiza que todas las reservas futuras puedan realizarse.
F2 se reabrió para probar el fallo de `munmap` durante `Drop`; esa prueba aislada
está implementada. La reaprobación del cierre queda pendiente de una decisión
explícita. Esto no inicia F3; el overflow real sobre el stack de una rutina
continúa pendiente de esa fase.
