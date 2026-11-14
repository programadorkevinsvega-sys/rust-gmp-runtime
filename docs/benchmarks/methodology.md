# Metodología de benchmarks

## Registro reproducible

Por benchmark conservar commit, `rustc -V`, hardware/CPU, kernel o SO, governor,
configuración, comando exacto, código de workload, calentamiento, al menos 10
repeticiones, datos brutos y variación. Reportar mediana, p99 y dispersión. No
inventar resultados ni comparar workloads con semánticas diferentes sin
explicarlas. Guardar informes en
`docs/benchmarks/results/<fecha>-<commit>.md`.

## Cargas y línea base

Preparar workloads para CPU pequeña, espera/suspensión, canales, carga mixta,
miles de tareas, red y carga sostenida. El benchmark usa `harness = false` y
`std::time::Instant`. Calentar antes de medir, ejecutar al menos 10 veces y
mantener el trabajo y la concurrencia comparables. Cuando aporte contexto,
comparar con threads nativos de Rust usando el mismo número de workers.

Las pruebas diferenciales deben usar un modelo de referencia determinista que
represente los contratos documentados de cierre de canales, selección,
despertar, cancelación y bloqueo. Documentar las divergencias respecto al
modelo; no afirmar equivalencia de comportamiento a partir de una sola cifra.

## Métricas

Spawn, ejecución, latencia/overhead de planificación, cambios de contexto,
throughput, CPU, espera/contención, RAM por tarea/total, espacio virtual
reservado, páginas comprometidas/tocadas, RSS, stacks/slabs/VMA, fragmentación,
fallos y tiempos de creación/planificación/finalización. Reportar por separado.

## Política

Primero medir y perfilar; optimizar solo cuellos de botella reproducibles y
medir de nuevo con el mismo método. Los parámetros 8 KiB iniciales y 256 KiB
virtuales de stack son candidatos de experimento, no defaults ni garantías.
Pruebas progresivas de escala objetivo: 1k, 10k, 100k, 500k y 1M G, solo cuando
los recursos existentes lo permitan. Registrar vm.max_map_count observado, sin
modificar límites globales.
