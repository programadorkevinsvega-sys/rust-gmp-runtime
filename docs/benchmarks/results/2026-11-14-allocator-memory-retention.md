# RSS y retención del pool experimental de stacks

## Alcance y entorno

Medición aislada del allocator Go-inspired experimental; no cambia el allocator
Fixed de producción ni modifica ADR 002. Se midieron dos estados de slots libres:
devolución al pool global y retención en ocho caches locales de Processor
simuladas. Cada muestra corre en un proceso hijo nuevo para obtener una línea
base RSS independiente.

- Intel Core i5-12450H, 12 CPUs lógicas, Linux 7.0.0-42-generic, governor
  `powersave`, `vm.max_map_count=1.048.576`.
- Rust `1.85.0 (4d91de4e4 2025-02-17)`.
- 8.192 stacks de 8 KiB útiles, 256 slots por span, 32 spans por muestra.
- Se tocó una dirección en **cada página usable** mientras cada lease seguía
  vivo: 64 MiB de páginas de stack tocadas por muestra.
- Diez procesos/muestras por cada combinación de estado y target de retención
  (80 muestras en total).
- Targets: 0, 1, 4 y 16 spans retenidos. En cada muestra se midió RSS después
  de tocar stacks, después de devolver los handles, después del flush local
  cuando aplica, después del trim y después de liberar todos los spans.
- El preflight de VMA permaneció activo; no se modificaron límites del sistema.

La [salida bruta](./runs/20261114T2050-allocator-memory-retention.log) conserva
las 80 muestras. Comando:

```sh
RUST_GMP_MEMORY_STACKS=8192 RUST_GMP_MEMORY_SAMPLES=10 \
  cargo +1.85.0 bench --bench allocator_memory --features allocator-benchmark
```

## Resultados

Mediana de RSS del proceso **por encima de su propia línea base**, en MiB
(1 MiB = 1.048.576 bytes). RSS viene de `/proc/self/statm` y es una observación
aproximada de todo el proceso, no una atribución exacta al allocator. El rango
entre muestras para las columnas de trim fue de 0–0,06 MiB alrededor de las
medianas mostradas.

| Estado de slots libres | Target spans | RSS tras tocar stacks | RSS tras devolver handles | RSS tras flush de caches | RSS tras trim | RSS tras liberar todo | Virtual reservado tras trim |
|---|---:|---:|---:|---:|---:|---:|---:|
| Pool global | 0 | 64,28 | 64,18 | — | 0,18 | 0,18 | 0 MiB |
| Pool global | 1 | 64,28 | 64,18 | — | 2,18 | 0,18 | 3 MiB |
| Pool global | 4 | 64,28 | 64,18 | — | 8,18 | 0,18 | 12 MiB |
| Pool global | 16 | 64,28 | 64,18 | — | 32,18 | 0,18 | 48 MiB |
| Caches locales | 0 | 64,34 | 64,50 | 64,50 | 0,50 | 0,18 | 0 MiB |
| Caches locales | 1 | 64,34 | 64,50 | 64,50 | 2,50 | 0,18 | 3 MiB |
| Caches locales | 4 | 64,34 | 64,50 | 64,50 | 8,50 | 0,18 | 12 MiB |
| Caches locales | 16 | 64,34 | 64,50 | 64,50 | 32,50 | 0,18 | 48 MiB |

El layout reservó 96 MiB antes del trim: 32 spans × 256 slots × (8 KiB
usable + 4 KiB guard). El RSS activo creció aproximadamente 64,3 MiB, coherente
con tocar las dos páginas usables de cada stack; los 32 MiB restantes eran
guard pages/espacio virtual y no se contabilizan como memoria residente. Las
medianas verificadas para memoria virtual después del trim fueron 0/3/12/48
MiB según el target.

Soltar los leases no desmapea ni descarta sus páginas: el RSS quedó alrededor
de 64,2–64,5 MiB. Vaciar las ocho caches tampoco redujo apreciablemente el RSS;
solo movió hasta 16 slots libres al pool global. Tras el trim, el RSS siguió
la cantidad retenida: aproximadamente 2, 8 y 32 MiB de páginas de stack para
1, 4 y 16 spans. `release_empty_spans()` liberó lo restante, y el RSS quedó a
aproximadamente 0,18 MiB de la línea base.

Esto confirma en el workload medido que la política de retención controla
directamente la memoria residente de spans reutilizables y que el allocator
puede devolver al sistema operativo las páginas tocadas al liberar los spans.
El proceso conserva un pequeño residuo de RSS; no se atribuye al pool. Los
resultados no prueban comportamiento de RSS bajo carga concurrente del runtime
ni una política óptima de retención.

## Límites para una decisión de producción

- `/proc/self/statm` es aproximado y mide el proceso completo. El baseline
  aislado reduce, pero no elimina, la limitación de atribución.
- Las caches son instancias experimentales coordinadas por el harness, no
  Processors reales ni el protocolo de quiescencia del scheduler.
- El punto de carga tocó todos los stacks una vez; no caracteriza escrituras
  posteriores, churn prolongado, presión de memoria del sistema ni varios
  tamaños de stack.
- Diez muestras por condición muestran poca dispersión en este host, pero no
  sustituyen replicación en otros kernels/hardware ni escenarios del runtime.

La medición cierra la brecha de RSS para este ciclo aislado de vida y retención.
Por sí sola no promueve Go-inspired ni cambia la decisión vigente de ADR 002:
la integración con Processors y el ciclo de quiescencia aún requieren su
validación propia.

## Validación

- `cargo +1.85.0 check --bench allocator_memory --features allocator-benchmark`: correcto.
- `cargo +1.85.0 test --features allocator-benchmark --lib -- --test-threads=1 --quiet`: 104 pasaron, 4 ignoradas.
- `cargo +1.85.0 fmt --all -- --check` y `git diff --check`: correctos.
