# Stack Management

## Requisitos

Cada G requiere stack independiente de dirección estable y liberación solo tras
finalizar toda posibilidad de uso. Los stacks no se copian ni reubican. Su owner
RAII permanece asociado a G; un waiter, handle o backend nativo pendiente debe
impedir su liberación.

## Motores de producción

`StackManager` conserva el motor Fixed como default y permite seleccionar
Go-inspired mediante `StackConfig::with_engine(StackEngine::GoInspired)`.
`Routine` usa la misma fachada y el mismo owner RAII para ambos; no duplica
contexto, ciclo de vida ni manejo de errores. Go-inspired crea stacks desde su
pool global en esta ruta. Las caches locales continúan siendo explícitas y no
están asociadas a los Processors reales: la creación de rutinas ocurre en el
caller, antes de su publicación.

El motor Go-inspired reutiliza slots dentro de spans y mantiene accesibles las
páginas de slots liberados. Los spans vacíos se retienen según la política del
motor; `StackManager::trim` y `trim_to_retained_spans` permiten recuperar spans
libres. No se liberan spans referenciados por leases o caches. Las métricas
comunes identifican el motor; `StackMetrics::go_inspired` conserva además las
métricas detalladas del pool y sus spans.

La selección es explícita: Fixed sigue siendo el default. La medición de hasta
500k stacks encontró retención importante de VMAs en Go-inspired hasta trim o
destrucción del manager; por ello su disponibilidad en producción no constituye
una recomendación de default ni una garantía de escalabilidad. Véanse
[ADR 016](./decisions/016-production-stack-engines.md) y el
[torneo de allocators](./benchmarks/results/2026-11-14-allocator-tournament.md).

## Motor Fixed: mapeo fijo, sin crecimiento

El motor Fixed reserva individualmente en Linux x86_64 una región
`PROT_NONE` con `mmap`, mantiene la primera página como guardia y habilita las
páginas restantes con `mprotect`. El tamaño solicitado se redondea a la página
del sistema; los límites expuestos permiten verificar alineación y tamaño. El
stack tiene dirección estable y su owner RAII lo libera con `munmap`.

No hay crecimiento automático, stack copying, recuperación por signal handler,
ni reutilización/pool. Tocar la guardia produce un fault no recuperado y termina
el proceso; no se interpreta como crecimiento. La implementación no fija un
tamaño operativo predeterminado, y no declara compatibilidad de stacks con ABI
de ejecución porque context switching aún no está implementado.

Cada reserva Fixed usa un mapeo independiente. El benchmark F2 midió
hasta 500k reservas (1,000,031 VMA en este entorno; el límite observado fue
1,048,576). Un experimento agrupado con un mapping y un `mprotect` por slot
redujo el tiempo de reserva, pero no redujo los VMA: a 500k observó 1,000,032.
El experimento no reemplaza el allocator ni mide liberación parcial, retención,
fragmentación o accounting. No se afirma escalabilidad de ejecución del
runtime y no se modifica `vm.max_map_count`.

## Diagnóstico y preflight de VMA

En Linux, `StackManager::vma_snapshot()` lee el límite real de
`/proc/sys/vm/max_map_count` y cuenta entradas validadas de `/proc/self/maps`.
El archivo se procesa en streaming con un buffer de 64 KiB reutilizable, y cada
entrada sigue validándose antes de contar. Un error de lectura o contenido
malformado se propaga, nunca se transforma en cero VMA ni en capacidad
ilimitada. `metrics()` requiere esta observación.

El preflight está habilitado por defecto y retiene 1.024 VMA como margen
configurable mediante `StackConfig::with_vma_safety_margin`. Puede deshabilitarse
explícitamente con `without_vma_preflight`; en ese modo las reservas siguen
serializadas y los fallos nativos se propagan, pero no se ofrece admisión
preventiva. Si procfs no está disponible con preflight habilitado, `allocate`
falla antes de mapear cuando necesita el snapshot inicial o cuando la llamada
actual inicia un refresco fallido. Durante un refresco periódico con un
snapshot previo, otras asignaciones pueden continuar usando esa observación
orientativa y el estimate interno actualizado; el error de refresco se propaga
a la llamada que lo inició.

En Fixed, la estimación máxima de dos VMA por stack se deriva de un
`mmap(PROT_NONE)` y un `mprotect` sobre la región usable, que puede dividir el
mapping. Go-inspired admite por span sus slots y usa la misma cota de hasta dos
VMA por slot activado. Ninguna cota es un conteo atribuible exacto: Linux puede
fusionar o dividir mappings. `runtime_mapping_count` cuenta mappings/span
retenidos por el motor; `runtime_vma_estimate` es una cota interna, mientras
`vma_current` es la lectura real del proceso. Fixed serializa preflight,
reserva/protección y
liberación para los managers clonados. La lectura y parseo de procfs se hacen
fuera de ese mutex. Durante un refresco periódico, las reservas usan el último
snapshot junto con un contador monótono de coste VMA de cada `mmap` exitoso
desde que inició la lectura; ese contador se actualiza bajo el mutex. No se
restan liberaciones concurrentes del snapshot: pueden quedar sobrecontadas
hasta la siguiente observación, evitando que una liberación aún no reflejada
cancele una reserva nueva que el snapshot no alcanzó a ver. Si aún no hay
snapshot, las reservas esperan el primero sin retener el mutex. Las consultas
explícitas de métricas esperan un snapshot fresco, también leído fuera del
mutex. El refresco ocurre como máximo cada 262.144 operaciones de lifecycle del
manager. Cada asignación calcula el presupuesto con el snapshot disponible y
los eventos de reserva posteriores al inicio de su lectura, sin releer procfs
por stack. La frecuencia y los snapshots son orientativos, no garantías contra
mappings externos; `mmap`/`mprotect` siempre pueden fallar y sus errores se
propagan.

El preflight es orientativo: no puede reservar capacidad del kernel frente a
otros mapeos concurrentes, cambios de límite o fusiones/divisiones distintas
del máximo estimado. Por ello, un `mmap` o `mprotect` todavía puede fallar tras
una admisión satisfactoria. Ese error se informa y se libera el mapping
parcial; si procfs falla no se afirma que haya capacidad.

Los valores de 8 KiB iniciales y 256 KiB reservados son únicamente candidatos
de benchmark, no defaults aprobados. Medir perfiles con `println!`/formateo,
recursión, locals grandes y profundidad. Esas cargas requieren ejecución sobre
el stack de G y se validarán después de F3; F2 no demuestra capacidad de
ejecución. Separar espacio virtual reservado, páginas tocadas/comprometidas y
RSS.

## Pools y liberación

Fixed no tiene pool y libera su mapping al desaparecer el último owner. En
Go-inspired el lease RAII devuelve el slot al pool global cuando desaparece su
último owner; el slot no se reutiliza antes de consumir el lease. Los mappings
de span pueden permanecer reservados con slots libres. Las referencias de
`Routine` y `ExecutionPermit` mantienen vivo cualquiera de los leases. Trim solo
libera spans sin leases ni slots cacheados; la retención de VMAs debe observarse
con las métricas del motor antes de elegirlo para cargas de alta cardinalidad.

## Alternativa experimental: Dynamic Growable Stack (no implementada)

Fixed y Go-inspired son los motores implementados; ambos mantienen tamaño fijo.
El diseño candidato independiente reserva un rango virtual estable hasta un
tamaño máximo configurable y habilita desde el extremo alto el tamaño inicial.
Un crecimiento explícito habilitaría páginas contiguas y movería la guardia; la
reducción solo podría ocurrir en un punto seguro cuando ningún frame, puntero o
referencia use las páginas que se van a proteger. No se copia ni reubica el
stack.

Reservar el máximo consume espacio virtual, no necesariamente memoria
residente. `mprotect(PROT_NONE)` protege páginas, pero no garantiza devolver
páginas residentes; una operación de decommit necesita decisión, API y medida
propias. El número de VMA depende de cómo Linux combine las regiones y debe
medirse. La contabilidad futura separará virtual reservado, páginas habilitadas,
estimación VMA del runtime, observación procfs y RSS aproximada.

Este diseño es preparación documental, no una API, allocator ni funcionalidad
disponible. El crecimiento explícito requiere contratos de safe-point y
pruebas de límites; el crecimiento automático al tocar una guardia necesita
F3: contexto ejecutable, política de stack pointer, handler y recuperación
validada. No se infiere crecimiento del kernel por demanda ni se cambia el
allocator fijo.

## Ownership, métricas y pruebas

Las referencias de `Routine` y `ExecutionPermit` mantienen vivo el stack hasta
que desaparece el último owner. El manager contabiliza asignaciones, fallos,
liberaciones, mapeos activos y bytes virtuales reservados/habilitados. Los bytes
habilitados representan espacio virtual con permisos de lectura/escritura, no
memoria residente ni páginas efectivamente tocadas. En Linux, RSS de
`/proc/self/statm` es aproximada y el conteo VMA proviene de `/proc/self/maps`;
`/proc/sys/vm/max_map_count` proporciona el límite actual, no una garantía de
que futuras reservas sean posibles.

Los fallos de reserva/protección se propagan con contexto; un fallo de liberación
en `Drop` es fatal porque no puede devolverse al llamador. Las métricas se
actualizan con contadores comprobados; inconsistencia durante la liberación
también es fatal.

Las pruebas actuales cubren alineación, stack pequeño, guardia en un proceso
hijo, direcciones no solapadas/estables, accounting y retención por ownership.
No existe reutilización que probar por la decisión inicial sin pool. La prueba
de guardia comprueba un fault en el límite, pero no demuestra overflow al
ejecutar código en el stack de una rutina. Recursión, variables locales grandes
y formato/`println!` requieren el context switching de F3.
