# ADR 002: Estrategia de stacks

- Estado: Aceptado para producción: stacks fijos y acotados, sin crecimiento ni
  reuso; estrategia dinámica experimental descrita abajo, aún no implementada ni
  autorizada para sustituir al stack fijo.
- Fecha: 2026-11-14.

## Problema

Las referencias y punteros pueden depender de las direcciones de stack; cada G stackful requiere una región propia con límites y liberación segura.

## Alternativas y decisión

Alternativas: stack de tamaño fijo; reserva virtual y compromiso de páginas bajo
demanda; crecimiento por page-fault/handler; stacks copiables; pools de slots.

La decisión inicial es capacidad fija por G, direcciones estables y guard page
inaccesible, sin stack copying/relocation, growth ni pool. Un overflow no se
recupera: el proceso falla de manera explícita antes de permitir corrupción.
El tamaño no se fija con números especulativos: F2 medirá uso real (incluidos
formateo, recursión y locals grandes), y F10 revisará la capacidad/performance.

La reserva por stack es el baseline de corrección para probar guardia y
ownership. El benchmark experimental F2 comparó reserva individual y agrupada
en tiempos de `mmap`/`mprotect` y VMA hasta 500k slots. La agrupada redujo el
tiempo de esa secuencia, pero produjo esencialmente los mismos VMA al conservar
un `mprotect` por slot. No se adopta una topología slab en producción: la
medición no incluye ciclo de vida, liberación parcial, retención ni garantías
completas de ownership. Antes de una futura adopción se requieren esos
benchmarks y una revisión de seguridad. No cambiar `vm.max_map_count`.

El Stack Manager incorpora diagnóstico en Linux de `/proc/sys/vm/max_map_count`
y `/proc/self/maps`, además de preflight serializado y margen configurable.
Con `mmap(PROT_NONE)` seguido por `mprotect` sobre la región usable, estima un
máximo de dos VMA nuevas por stack; el kernel puede fusionar regiones, por lo
que esta cota no es el número exacto de VMAs del runtime. El preflight no
garantiza capacidad futura frente a mappings externos concurrentes. Esta
protección no cambia la topología ni autoriza adoptar slabs.

## Evaluación pendiente de slabs (sin adopción)

| Propiedad | Evidencia actual | Lo que faltaría para decidir |
|---|---|---|
| VMAs por slab | El prototipo hizo un mapping común y `mprotect` por slot; observó aproximadamente 2 VMAs por slot más una para el mapping base, no una VMA por slab. | Repetir con lifecycle real, procesos/entornos distintos y disposiciones mixtas; el kernel puede fusionar/dividir regiones. |
| Slots y guard regions | Se midieron lotes de 1k, 10k, 100k y 500k slots; el layout reservado/protegido conserva la guardia por slot. | Definir un tamaño de slab limitado y demostrar alineación de cada slot a la página. |
| Fragmentación y retención | No se midieron con el prototipo. | Medir slots libres dispersos, slabs parcialmente ocupados y umbral de devolución de slab al SO. |
| Liberación y reutilización | El prototipo solo midió reserva/protección; producción libera el mapping completo por stack y no reutiliza. | Probar liberación/reuso de slot, limpieza de permisos, ownership y ausencia de referencias activas. |
| Virtual, comprometido y RSS | El benchmark de topología no tocó páginas ni midió liberación; son magnitudes distintas. | Medir memoria virtual reservada, páginas tocadas/comprometidas y RSS durante ocupación y liberación parcial. |
| Creación/destrucción y concurrencia | Se comparó tiempo de reserva; no se midió destrucción del slab ni una free list concurrente. | Medir creación/destrucción y verificar un lock/bitmap de slots ante carreras, doble asignación y use-after-free. |

No se justifica introducir slabs en producción con los datos actuales. La
decisión requiere completar las mediciones de ciclo de vida y una revisión de
seguridad; el prototipo no constituye un allocator ni modifica ADR 002.

## Alternativa experimental: Dynamic Growable Stack

El stack fijo permanece como valor predeterminado y como implementación de
producción. Se prepara una alternativa independiente para experimentación
posterior; esta descripción no afirma que exista código de crecimiento.

Diseño candidato:

- Reservar de una vez un rango virtual contiguo de tamaño máximo con dirección
  estable y permisos `PROT_NONE`. El stack crece hacia abajo, específico de la
  primera ABI objetivo, sin copiar ni reubicar frames.
- Configurar tamaño inicial y máximo, ambos alineados a página, con
  `0 < inicial <= máximo`, límites comprobados y presupuesto de VMA/bytes
  validado antes de reservar. La reserva completa consume espacio virtual desde
  el inicio; no equivale a memoria residente o comprometida.
- Habilitar explícitamente páginas contiguas adyacentes al límite inferior
  activo mediante `mprotect`, moviendo la guardia para que permanezca entre la
  región usable y la reserva todavía inaccesible. El layout exacto de regiones
  y sus VMA se debe medir en Linux; no inferir un conteo fijo por stack.
- Permitir reducción solo en un punto seguro del scheduler, con el stack
  suspendido y prueba de que el stack pointer, los frames, referencias y
  operaciones pendientes quedan fuera de las páginas que se deshabilitan.
  `mprotect(PROT_NONE)` por sí solo no promete devolver páginas residentes al
  SO; cualquier `madvise`/decommit requerirá contrato, FFI aislada y medición.
- Mantener contabilidad separada de reserva virtual máxima, páginas habilitadas,
  mappings propios, estimación VMA, RSS aproximada, crecimiento/reducción y
  fallos. Ownership exclusivo y mutex protegen transiciones; no introducir
  lock-free ni pool/slab.
- Fallos al crecer dejan sin cambios el límite anterior y los permisos
  existentes; fallos de reducción no pueden invalidar frames activos. Overflow
  automático por page fault queda fuera de esta infraestructura.

La infraestructura de reserva, habilitación explícita y reducción es una
capacidad futura de memoria, no ejecución automática. El manejador de page fault,
la lectura/validación del stack pointer activo, la entrada/salida de señal y la
recuperación segura requieren context switching y control de ejecución de F3;
no se simularán en F2. La implementación experimental requiere un ADR de
implementación que especifique API de safe-point, dirección de crecimiento,
guardia, ABI, unwind/panic, límites, errores parciales y contabilidad, además
de pruebas de protección/overflow aisladas y benchmarks comparativos contra
fixed stack. No cambiar el allocator actual ni el default sin aprobación
explícita, revisión de seguridad y evidencia completa de ciclo de vida.

## Costes de CPU y memoria

Stack fijo simplifica correctitud pero puede reservar espacio virtual no usado y
limitar profundidad. Guard/fault handling añade coste y riesgo; pools retienen
memoria y añaden limpieza/sincronización. Medir reserva, páginas tocadas/RSS,
syscalls, VMA, creación, liberación y retención por separado.

## Riesgos, límites y validación

Los valores 8 KiB inicial y 256 KiB reservado solo son candidatos de benchmark.
Pruebas F2/F12 deben aislar el overflow fatal, probar guardia, alineación,
depth/locals, liberación y errores de reserva. F2 midió reservas hasta 500k;
1M no fue posible con el límite VMA observado y queda condicionado a recursos.
Depth/locals y overflow durante ejecución se validarán con el context switching
real. No declarar crecimiento ni escalabilidad de ejecución no medida.
