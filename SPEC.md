RUST GMP RUNTIME
MASTER ENGINEERING SPECIFICATION
Diseño, arquitectura e implementación de un runtime de concurrencia M:N desarrollado nativamente en Rust.

Tipo de proyecto: Greenfield Systems Programming
Lenguaje: Rust
Arquitectura: G-M-P Scheduler
Modelo de ejecución: M:N Stackful Green Threads
Dependencias externas: Ninguna
Primera plataforma: Linux x86_64
Arquitecturas futuras: Linux AArch64, macOS y Windows
Objetivo: Alto rendimiento, bajo consumo de RAM y CPU, seguridad de memoria, escalabilidad y mantenibilidad.

PARTE I — IDENTIDAD Y OBJETIVOS DEL PROYECTO
1. Rol del agente de desarrollo

Actúa como un ingeniero principal de sistemas (Principal Systems Engineer), especializado en:

Desarrollo de runtimes y máquinas virtuales.
Diseño de schedulers.
Sistemas operativos.
Arquitectura de concurrencia G-M-P.
Programación de bajo nivel con Rust.
Assembly y ABI de procesadores.
FFI y APIs nativas de sistemas operativos.
Context switching.
Gestión de stacks.
Gestión de memoria virtual.
Estructuras concurrentes.
Algoritmos lock-free.
Sincronización.
Networking de bajo nivel.
Optimización de CPU y memoria.
Ingeniería de rendimiento.

Tu responsabilidad es diseñar, documentar e implementar un runtime de concurrencia propio para Rust, usando G-M-P como modelo arquitectónico.

No debes construir un wrapper de una librería existente.

No debes construir un Thread Pool convencional y presentarlo como un runtime de rutinas.

Debes implementar un motor de ejecución propio capaz de multiplexar tareas ligeras sobre un conjunto limitado de threads del sistema operativo.

El resultado debe ser un runtime real, modular, robusto, escalable y técnicamente verificable.

2. Visión del proyecto

Desarrollar un runtime M:N stackful que permita ejecutar funciones síncronas de Rust como unidades de ejecución ligeras.

El usuario final deberá poder crear miles de rutinas sin crear un thread del sistema operativo por cada una.

La arquitectura usará el modelo G-M-P:

G: Rutina.
M: Machine.
P: Processor.

El runtime administrará directamente la ejecución, planificación, suspensión, reanudación, memoria y sincronización de las rutinas.

La API pública deberá ser sencilla e idiomática para Rust.

Ejemplo del objetivo final:

rust
use rust_gmp_runtime::Runtime;

fn main() {
    let runtime = Runtime::new(4);

    for i in 0..10_000 {
        runtime.spawn(move || {
            println!("Rutina: {}", i);
        });
    }

    runtime.wait();
}


El usuario no tendrá que utilizar:

async.
.await.
Tokio.
Axum.
Un executor externo.
Un runtime externo.

La concurrencia será administrada por nuestro propio motor de ejecución.

3. Objetivos técnicos obligatorios
Implementar un scheduler G-M-P.
Implementar rutinas stackful.
Ejecutar miles de rutinas sobre pocos threads.
Implementar context switching propio.
Implementar gestión de stacks independientes.
Implementar suspensión y reanudación.
Implementar colas locales y globales.
Implementar Work Stealing.
Implementar balanceo de carga.
Implementar mecanismos de preemption.
Implementar canales.
Implementar sincronización.
Implementar temporizadores.
Implementar un Netpoller propio.
Implementar gestión eficiente de memoria.
Garantizar la seguridad de las operaciones concurrentes.
Minimizar el consumo de CPU.
Minimizar el consumo de RAM.
Evitar asignaciones innecesarias.
Mantener una arquitectura extensible.

No sacrificar la seguridad de memoria para conseguir rendimiento.

No afirmar que una característica está implementada si solamente existe su interfaz.

PARTE II — RESTRICCIONES Y TECNOLOGÍAS
4. Dependencias: cero crates externos

Esta es una restricción absoluta del proyecto.

No utilizar dependencias externas.

Está prohibido utilizar:

Tokio.
Axum.
async-std.
smol.
Rayon.
Crossbeam.
Parking Lot.
Futures.
Flume.
DashMap.
Crates de context switching.
Crates de gestión de stacks.
Crates de networking asíncrono.
Crates de sincronización.
Cualquier runtime externo.
Cualquier biblioteca externa que implemente los componentes centrales.

No introducir dependencias externas indirectamente mediante otros crates.

No utilizar crates para resolver funcionalidades que debemos implementar nosotros.

5. Tecnologías permitidas

Utilizar exclusivamente las capacidades nativas de Rust, su biblioteca estándar y las interfaces nativas del sistema operativo.

Rust Standard Library
Módulo	Responsabilidad
std::thread	Creación y administración de threads
std::sync	Sincronización
std::sync::atomic	Operaciones atómicas
std::cell	Mutabilidad interna
std::alloc	Asignación dinámica
std::ptr	Operaciones de punteros
std::mem	Administración de memoria
std::time	Tiempo y temporizadores
std::collections	Estructuras de datos
std::net	Networking
std::io	Entrada y salida
std::arch	Operaciones específicas de arquitectura
std::hint	Optimizaciones
std::panic	Gestión de pánicos cuando sea compatible con el modelo de ejecución
Componentes fundamentales

También se permite:

core.
alloc.
Inline Assembly.
global_asm!.
asm!.
Declaraciones FFI mediante extern.
APIs nativas de Linux, Windows y macOS.
Assembly específico de arquitectura.

El uso de FFI y Assembly no constituye una dependencia externa de Rust.

Sin embargo, cualquier llamada al sistema operativo debe estar aislada y documentada.

Restricción adicional

No depender de:

libc como crate.
Bindings externos.
Bibliotecas de terceros.
Runtime de C++.
Frameworks de concurrencia.

Cuando necesitemos funcionalidades nativas, implementar las declaraciones FFI mínimas necesarias y documentar sus contratos ABI.

No inventar firmas FFI.

Verificar las firmas, tipos, convenciones de llamada y requisitos de cada sistema operativo.

PARTE III — ARQUITECTURA GENERAL
6. Arquitectura G-M-P

Implementar una arquitectura propia con el modelo G-M-P.

G — Rutina

Representa una unidad independiente de ejecución.

Cada G debe disponer de:

Identificador único.
Estado.
Contexto de CPU.
Stack independiente.
Información de planificación.
Información de bloqueo.
Información de finalización.
Metadatos mínimos.

Una rutina debe poder suspenderse y reanudarse conservando su estado de ejecución.

M — Machine

Representa un thread del sistema operativo.

Responsabilidades:

Ejecutar rutinas.
Mantener el contexto del thread.
Asociarse con un Processor.
Participar en el scheduler.
Administrar cambios de contexto.
Gestionar el bloqueo y desbloqueo.
Ejecutar el código de las rutinas.

El número de Machines no debe crecer proporcionalmente al número de rutinas.

P — Processor

Representa los recursos lógicos necesarios para ejecutar rutinas.

Responsabilidades:

Administrar una cola local.
Obtener rutinas ejecutables.
Distribuir trabajo.
Participar en Work Stealing.
Asociarse temporalmente con una Machine.
Administrar el estado de ejecución.
Participar en el balanceo de carga.

La cantidad inicial de processors será configurable.

Como configuración predeterminada, utilizar el número de CPUs lógicas disponibles cuando pueda determinarse de manera fiable.

7. Arquitectura lógica

Implementar los siguientes niveles:

text
                 PUBLIC API
                      |
                      v
                RUNTIME CORE
                      |
          +-----------+-----------+
          |                       |
          v                       v
      SCHEDULER              TASK MANAGER
          |                       |
          +-----------+-----------+
                      |
                      v
                 G-M-P ENGINE
                      |
        +-------------+-------------+
        |             |             |
        v             v             v
   PROCESSORS      MACHINES      RUTINAS
        |             |             |
        +-------------+-------------+
                      |
                      v
                EXECUTION ENGINE
                      |
        +-------------+-------------+
        |             |             |
        v             v             v
   STACK MANAGER  CONTEXT SWITCH  PREEMPTION
        |             |             |
        +-------------+-------------+
                      |
                      v
                 SUPPORT SYSTEMS
                      |
       +--------------+--------------+
       |              |              |
       v              v              v
    CHANNELS        SYNC          TIMERS
       |
       v
    NETPOLLER
                      |
                      v
                 PLATFORM LAYER
                      |
             OS APIs / ASSEMBLY

PARTE IV — SUBSISTEMAS
8. Runtime Core

El Runtime Core será el punto de entrada y coordinación.

Responsabilidades:

Inicialización.
Configuración.
Administración del ciclo de vida.
Creación de rutinas.
Administración de processors.
Administración de machines.
Apagado ordenado.
Control de recursos.
Administración de errores.
Estadísticas generales.

Implementar un constructor configurable:

rust
let runtime = Runtime::builder()
    .processors(4)
    .build();


Las opciones definitivas deben considerar los recursos reales disponibles.

El runtime debe evitar inicializaciones costosas cuando no sean necesarias.

9. Scheduler

El Scheduler será el corazón del proyecto.

Debe implementar:

Global Run Queue

Cola global de rutinas ejecutables.

Debe utilizarse cuando:

Se generan tareas desde contextos externos.
Las colas locales necesitan trabajo.
Es necesario redistribuir tareas.
Se requiere balanceo global.
Local Run Queue

Cada Processor debe disponer de una cola local.

Objetivos:

Reducir contención.
Minimizar sincronización global.
Mejorar localidad de memoria.
Reducir operaciones atómicas.
Aumentar el throughput.
Work Stealing

Implementar robo de tareas entre processors.

Cuando un Processor no tenga trabajo:

Buscar trabajo global.
Intentar obtener trabajo de otro Processor.
Robar una cantidad controlada de tareas.
Evitar competir constantemente por las mismas colas.
Volver a un estado de espera eficiente si no hay trabajo.

Diseñar correctamente la sincronización y propiedad de las tareas.

No utilizar estructuras lock-free sin demostrar sus invariantes y seguridad.

La primera implementación de las colas debe priorizar estructuras convencionales sincronizadas con primitivas de `std`, por ejemplo `Mutex`, y no algoritmos lock-free. No mantener un bloqueo mientras una rutina se suspende, cede el control o realiza I/O. Medir contención y latencia antes de optimizar. Solo considerar algoritmos como Chase-Lev tras justificar su necesidad, revisar formalmente ownership, ABA, ordenamientos de memoria, liberación y carreras, y añadir pruebas pertinentes.

Scheduler Loop

Cada Machine asociada a un Processor deberá:

Obtener una rutina ejecutable.
Marcarla como Running.
Transferir el control al contexto de ejecución.
Recuperar el control cuando la rutina ceda, termine o se bloquee.
Actualizar su estado.
Seleccionar la siguiente rutina.

Evitar polling activo.

Utilizar mecanismos de parking y notificación cuando no exista trabajo.

10. Gestor de rutinas

Implementar un administrador de rutinas.

Estados:

rust
enum RutinaState {
    Created,
    Runnable,
    Running,
    Waiting,
    Syscall,
    Dead,
}


Las transiciones deben estar controladas.

Ejemplo:

text
Created
   |
   v
Runnable
   |
   v
Running
   |
   +------> Waiting
   |           |
   |           v
   |        Runnable
   |
   +------> Runnable
   |
   +------> Dead


Garantizar:

Una rutina no puede ejecutarse simultáneamente en dos processors.
Una rutina bloqueada no debe aparecer en una cola ejecutable.
Una rutina terminada no puede volver a ejecutarse.
No liberar su stack mientras exista una operación que pueda utilizarlo.
Las transiciones deben ser atómicas cuando corresponda.

Implementar identificadores eficientes y evitar metadatos excesivos.

11. Machine Manager

Implementar:

Creación de workers.
Registro de threads.
Asociación M-P.
Desasociación.
Estados de las Machines.
Parking.
Unparking.
Administración de threads auxiliares.
Control de apagado.

Cuando una Machine entre en una operación bloqueante del sistema operativo, estudiar la posibilidad de liberar temporalmente su Processor para que otra Machine pueda continuar ejecutando rutinas.

No confundir bloqueo de una rutina con bloqueo de una Machine.

PARTE V — EXECUTION ENGINE
12. Stack Manager

Implementar un administrador de stacks independientes.

Cada rutina deberá disponer de un stack propio.

Objetivos:

Consumo reducido de memoria.
Reserva eficiente.
Protección ante desbordamientos.
Liberación segura.
Reutilización.
Alineación correcta.
Compatibilidad con context switching.
Estrategia de memoria

No reservar una cantidad excesiva de memoria física para cada rutina.

Evaluar:

Reserva de espacio virtual.
Compromiso progresivo de páginas.
Guard pages.
Pools de stacks.
Reutilización de bloques.
Límites configurables.

En plataformas que lo permitan, utilizar memoria virtual reservada y comprometer páginas según se necesiten.

Los valores de 8 KiB inicialmente residentes/comprometidos y 256 KiB reservados pueden evaluarse como parámetros experimentales, nunca como garantías ni valores universales. Medir tamaños de página, RSS y límites reales por plataforma. Reservar espacio virtual y dejar que el sistema operativo materialice páginas al tocarlas no equivale por sí mismo a un mecanismo de crecimiento dinámico controlado.

Una guard page detecta ciertos accesos fuera del límite, pero no recupera por sí sola el stack ni hace seguro su crecimiento. La implementación inicial deberá elegir y documentar explícitamente entre stacks de tamaño efectivo fijo con overflow fatal/controlado, o una estrategia de crecimiento con manejo de fallos de página demostrada como segura. No declarar stack growing hasta probar guard relocation/commit, concurrencia, señales/fault handling, límites y recuperación. Considerar consumo de `println!`, formateo, recursión, variables locales grandes y llamadas profundas; un stack pequeño debe rechazarse o fallar de forma documentada, no corromper memoria.

Restricción fundamental de Rust

No mover arbitrariamente un stack activo.

Las referencias y punteros pueden depender de direcciones estables.

La arquitectura debe preservar las direcciones de los stacks durante su vida útil.

No implementar stack copying o stack relocation sin un mecanismo formal que garantice la validez de todas las referencias, punteros y objetos internos.

Inicialmente, utilizar stacks de direcciones estables con reserva virtual y límites explícitos. El crecimiento de memoria solo se habilitará después de implementar y validar un mecanismo seguro; si no está disponible, los stacks tendrán tamaño efectivo fijo y una política de overflow documentada.

Documentar las diferencias entre reserva virtual y memoria física residente.

13. Context Switching

Este subsistema es obligatorio.

Rust no proporciona una API estándar para cambiar entre stacks independientes.

Debemos desarrollar nuestro propio mecanismo utilizando Rust y Assembly.

Tecnologías
Rust.
Inline Assembly.
global_asm!.
FFI cuando sea necesario.
Código específico de arquitectura.
Responsabilidades
Guardar el contexto de CPU.
Restaurar el contexto.
Cambiar el stack pointer.
Preservar los registros exigidos por la ABI.
Mantener la alineación de stack.
Preservar el estado de ejecución.
Garantizar que el retorno al contexto anterior sea correcto.

Implementar interfaces internas que permitan aislar las particularidades de cada arquitectura.

No asumir que x86_64 y AArch64 utilizan el mismo convenio de registros.

Seguridad

Cada operación de context switching debe documentar:

Registros preservados.
Registros volátiles.
Convención de llamada.
Alineación.
Estado de stack.
Propiedad de memoria.
Requisitos de CPU.
Invariantes de seguridad.

No permitir que el control de ejecución salga de un contexto sin que el runtime conozca el estado de la rutina.

Panic y unwinding

Diseñar una estrategia explícita para gestionar pánicos.

No permitir que un unwinding atraviese arbitrariamente fronteras de context switching o frames de Assembly sin soporte formal.

Inicialmente, capturar los pánicos en el límite de ejecución de cada rutina cuando sea seguro hacerlo, o utilizar una política de abort explícita.

Documentar las limitaciones.

Cada rutina deberá entrar mediante un trampoline controlado que ejecute el cuerpo bajo `catch_unwind` cuando el crate use `panic = "unwind"`. El resultado (éxito o panic) se registrará en el estado de finalización y se devolverá al scheduler sin permitir que el unwinding atraviese el límite Assembly/context-switch. Definir qué ocurre con el valor de panic, destructores, recursos adquiridos y tareas dependientes; el descarte de un payload que a su vez haga panic debe contemplarse. `catch_unwind` no vuelve seguros stacks/ABI no válidos ni garantiza por sí solo que el unwinder pueda operar sobre una pila alternativa. En `panic = "abort"` no hay captura recuperable: el proceso termina. Probar ambas políticas/configuraciones donde sean soportables y no prometer continuidad del scheduler sin pruebas.

14. FFI y Assembly

Crear una frontera de bajo nivel claramente aislada.

La lógica de negocio del runtime debe permanecer en Rust.

Utilizar Assembly solamente cuando sea necesario.

Utilizar FFI para integrar APIs nativas del sistema operativo.

Ejemplos de responsabilidades:

Tecnología	Función
Rust	Scheduler y administración de tareas
Assembly	Context switching
FFI	Integración con APIs nativas
OS APIs	Memoria virtual y polling
Rust Atomics	Sincronización

No utilizar FFI innecesariamente.

Preferir `std::thread`, `std::sync`, `std::time`, `std::net` y otras APIs de `std` cuando cumplan los requisitos. Usar FFI/syscalls únicamente cuando no exista una alternativa suficiente (por ejemplo, memoria virtual protegida o backend de polling nativo), manteniéndolos en la capa de plataforma. No implementar creación de threads con `clone` ni syscalls equivalentes por cuenta propia: deben conservarse las inicializaciones, TLS y garantías de la biblioteca estándar de Rust.

Para Linux x86_64, evaluar por operación si se requiere interfaz libc o syscall directa. Documentar la fuente normativa, número de syscall si aplica, ABI, registros, argumentos, clobbers, retorno y conversión de errores. Las convenciones del kernel no son automáticamente las de libc. Revisar individualmente `mmap`, `mprotect`, `munmap`, `epoll_create1`, `epoll_ctl`, `epoll_wait`, `futex` y señales; no implementar las que `std` resuelve adecuadamente o que el runtime no necesita. Las syscalls de threads y señales/preemption asíncrona requieren justificación y diseño de seguridad propios.

Cada llamada nativa debe tener:

Firma correcta.
ABI correcta.
Tipos compatibles.
Gestión de errores.
Documentación.
Validación de parámetros.
15. Preemption

Implementar mecanismos de preemption progresivamente.

Diferenciar:

Preemption cooperativa

Una rutina cede voluntariamente el processor.

Preemption en puntos seguros

El runtime permite interrumpir la ejecución en puntos donde se hayan establecido invariantes seguras.

Preemption asíncrona

Investigar e implementar cuando sea técnicamente viable mediante mecanismos específicos de arquitectura y plataforma.

No afirmar que existe preemption asíncrona real cuando solamente se está utilizando un mecanismo cooperativo.

La preemption debe evitar que una rutina monopolice indefinidamente un Processor.

No utilizar sleeps artificiales como sustituto de preemption.

Documentar qué operaciones pueden retrasar la preemption.

PARTE VI — SINCRONIZACIÓN Y COMUNICACIÓN
16. Synchronization System

Implementar primitivas propias:

Mutex.
RWMutex.
Semaphore.
WaitGroup.
Condition Variable.

Las operaciones de bloqueo deben integrarse con el scheduler.

Cuando una rutina espere por un recurso, debe poder pasar a Waiting y ceder el Processor.

No bloquear innecesariamente el thread del sistema operativo.

Implementar correctamente:

Despertar.
Cancelación.
Orden de espera.
Liberación.
Condiciones de carrera.
Operaciones atómicas.

No implementar spinlocks como mecanismo predeterminado.

Los spinlocks solo podrán utilizarse cuando benchmarks y análisis demuestren una ventaja real.

17. Channel System

Implementar canales propios con semántica definida por la especificación del runtime.

API objetivo:

rust
let (tx, rx) = channel::<i32>();

spawn(move || {
    tx.send(100);
});

let value = rx.recv();


Características:

Unbuffered Channels

El envío y recepción deben sincronizarse directamente.

El emisor deberá esperar cuando no exista un receptor disponible.

Buffered Channels

Implementar buffers circulares eficientes.

Evitar reasignaciones innecesarias.

Operaciones
Send.
Receive.
Close.
TrySend.
TryReceive.
Select.
Administración
Waiters.
Productores.
Consumidores.
Bloqueo.
Desbloqueo.
Estados del canal.
Liberación de recursos.

Garantizar que las rutinas bloqueadas cedan el Processor.

Diseñar correctamente el comportamiento ante cierre y múltiples operaciones simultáneas.

18. Select

Implementar una operación de selección múltiple sobre canales, con semántica definida por la especificación del runtime.

Debe permitir esperar sobre:

Varios canales.
Operaciones de recepción.
Operaciones de envío.
Timeout.
Operaciones disponibles.

Garantizar que una rutina no se registre de forma incorrecta en múltiples operaciones y termine despertándose varias veces por una única espera.

Documentar las reglas de selección y equidad.

PARTE VII — TIMERS Y NETWORKING
19. Timer System

Implementar un sistema centralizado.

Funciones:

Sleep.
Timeout.
Deadline.
Timers periódicos.
Cancelación.
Despertar de rutinas.

Evaluar:

Binary Heap.
Timer Wheel.
Estructuras jerárquicas.

Elegir según benchmarks, consumo de memoria y patrones de uso.

Evitar crear un thread independiente por temporizador.

Una rutina en Sleep debe quedar suspendida sin ocupar innecesariamente un worker.

20. Netpoller

Implementar un subsistema propio de I/O no bloqueante.

El objetivo es integrar operaciones de red con el scheduler.

Crear una abstracción interna para:

Registrar descriptores.
Eliminar registros.
Esperar eventos.
Despertar rutinas.
Administrar timeouts.
Administrar conexiones.
Adaptadores

Linux:

epoll.

macOS:

kqueue.

Windows:

IOCP.

La implementación deberá utilizar APIs nativas mediante FFI.

No utilizar crates externos.

No simular I/O no bloqueante utilizando polling activo.

Cuando una operación de I/O espere un evento, la rutina debe pasar a Waiting y el Processor debe quedar disponible.

Distinguir correctamente entre operaciones de red, archivos y operaciones que el sistema operativo no permite multiplexar de la misma forma.

La interfaz del netpoller no debe modelar todas las plataformas como readiness. Separar registro, notificación de disponibilidad, operación pendiente, finalización, asociación con G, cancelación, error y wake-up. Linux `epoll` y macOS `kqueue` se tratarán como readiness-based; Windows IOCP como completion-based, con ciclo de vida y carreras propios. Un evento de readiness no equivale a que la operación ya se haya completado.

No asumir que `epoll` admite archivos regulares. Especificar y probar el comportamiento por tipo de descriptor (socket, pipe y archivo regular). Para operaciones que puedan bloquear, diseñar handoff/liberación del P, recuperación, recursos pendientes y apagado. Evaluar workers auxiliares acotados o una estrategia nativa adecuada; no crear threads ilimitados ni permitir que una operación de archivo bloquee todo el scheduler.

PARTE VIII — GESTIÓN DE MEMORIA
21. Memory Manager

Implementar una administración eficiente de memoria.

Objetivos:

Minimizar asignaciones.
Minimizar fragmentación.
Reducir contención del allocator.
Reutilizar objetos.
Reducir metadatos.
Mantener localidad de memoria.
Evitar copias.
Liberar recursos de forma determinista.

Evaluar:

Slab Allocator

Para estructuras pequeñas y de tamaño fijo.

Arena Allocator

Para asignaciones con ciclo de vida común.

Object Pools

Para reutilizar estructuras internas del runtime.

Stack Pools

Para reutilizar bloques de stacks liberados.

No reemplazar el allocator global de Rust sin una justificación técnica y benchmarks.

Evitar mantener memoria reservada indefinidamente cuando ya no sea necesaria.

No implementar Garbage Collector.

Utilizar ownership, borrowing, RAII y tipos seguros de Rust.

PARTE IX — PLATAFORMAS
22. Platform Layer

Separar completamente las implementaciones específicas del sistema operativo.

Primera plataforma:

Linux x86_64.

Plataformas futuras:

Linux AArch64.
macOS x86_64.
macOS AArch64.
Windows x86_64.
Windows AArch64 cuando sea viable.

No implementar todos los adaptadores simultáneamente.

Mantener interfaces independientes de la plataforma.

Cada plataforma deberá implementar sus mecanismos de:

Memoria virtual.
Protección de memoria.
Context switching.
Thread management.
I/O polling.
Despertar.
Primitivas del sistema operativo.

Desde la arquitectura inicial, representar explícitamente la diferencia entre backends readiness y completion, sin crear implementaciones stub para plataformas futuras. Mantener interfaces simples y definirlas a partir de las necesidades reales de Linux x86_64.

No afirmar compatibilidad con una plataforma si no se ha compilado y probado.

PARTE X — ARQUITECTURA DE CARPETAS
23. Estructura definitiva del proyecto

Utilizar la siguiente arquitectura modular:

text
rust-gmp-runtime/
│
├── Cargo.toml
├── README.md
├── LICENSE-APACHE
├── LICENSE-MIT
├── rust-toolchain.toml
├── SPEC.md
├── PHASES.md
│
├── docs/
│   ├── architecture.md
│   ├── gmp-model.md
│   ├── scheduler.md
│   ├── execution-engine.md
│   ├── context-switching.md
│   ├── stack-management.md
│   ├── memory-model.md
│   ├── synchronization.md
│   ├── channels.md
│   ├── timers.md
│   ├── networking.md
│   ├── platform-support.md
│   ├── security.md
│   ├── performance.md
│   ├── panic-and-unwinding.md
│   ├── tls-and-thread-affinity.md
│   ├── syscalls.md
│   ├── netpoller.md
│   ├── io-strategy.md
│   ├── security-invariants.md
│   ├── phases/
│   │   └── phase-1-report.md
│   │
│   ├── decisions/
│   │   ├── 001-gmp-model.md
│   │   ├── 002-stack-strategy.md
│   │   ├── 003-context-switching.md
│   │   ├── 004-scheduler-design.md
│   │   ├── 005-memory-management.md
│   │   └── 006-platform-abstraction.md
│   │
│   └── benchmarks/
│       └── methodology.md
│
├── src/
│   │
│   ├── lib.rs
│   ├── error.rs
│   ├── config.rs
│   │
│   ├── runtime/
│   │   ├── mod.rs
│   │   ├── runtime.rs
│   │   ├── builder.rs
│   │   ├── lifecycle.rs
│   │   └── shutdown.rs
│   │
│   ├── scheduler/
│   │   ├── mod.rs
│   │   ├── scheduler.rs
│   │   ├── dispatch.rs
│   │   ├── balance.rs
│   │   ├── work_stealing.rs
│   │   ├── preemption.rs
│   │   └── parking.rs
│   │
│   ├── routine/
│   │   ├── mod.rs
│   │   ├── routine.rs
│   │   ├── state.rs
│   │   ├── id.rs
│   │   ├── lifecycle.rs
│   │   └── metadata.rs
│   │
│   ├── processor/
│   │   ├── mod.rs
│   │   ├── processor.rs
│   │   ├── local_queue.rs
│   │   └── processor_pool.rs
│   │
│   ├── machine/
│   │   ├── mod.rs
│   │   ├── machine.rs
│   │   ├── worker.rs
│   │   └── thread_manager.rs
│   │
│   ├── stack/
│   │   ├── mod.rs
│   │   ├── stack.rs
│   │   ├── allocator.rs
│   │   ├── pool.rs
│   │   ├── growth.rs
│   │   └── protection.rs
│   │
│   ├── context/
│   │   ├── mod.rs
│   │   ├── context.rs
│   │   ├── switch.rs
│   │   └── arch/
│   │       ├── mod.rs
│   │       ├── x86_64.rs
│   │       └── aarch64.rs
│   │
│   ├── queue/
│   │   ├── mod.rs
│   │   ├── global.rs
│   │   ├── local.rs
│   │   ├── injection.rs
│   │   └── overflow.rs
│   │
│   ├── sync/
│   │   ├── mod.rs
│   │   ├── mutex.rs
│   │   ├── rwmutex.rs
│   │   ├── semaphore.rs
│   │   ├── wait_group.rs
│   │   └── condvar.rs
│   │
│   ├── channel/
│   │   ├── mod.rs
│   │   ├── channel.rs
│   │   ├── buffer.rs
│   │   ├── sender.rs
│   │   ├── receiver.rs
│   │   ├── waiter.rs
│   │   └── select.rs
│   │
│   ├── timer/
│   │   ├── mod.rs
│   │   ├── timer.rs
│   │   ├── wheel.rs
│   │   └── timer_queue.rs
│   │
│   ├── netpoll/
│   │   ├── mod.rs
│   │   ├── poller.rs
│   │   ├── registration.rs
│   │   └── platform/
│   │       ├── mod.rs
│   │       ├── linux.rs
│   │       ├── macos.rs
│   │       └── windows.rs
│   │
│   ├── memory/
│   │   ├── mod.rs
│   │   ├── allocator.rs
│   │   ├── arena.rs
│   │   ├── slab.rs
│   │   └── metrics.rs
│   │
│   ├── ffi/
│   │   ├── mod.rs
│   │   ├── memory.rs
│   │   ├── threads.rs
│   │   └── os.rs
│   │
│   ├── platform/
│   │   ├── mod.rs
│   │   ├── thread.rs
│   │   ├── atomic.rs
│   │   ├── syscall.rs
│   │   ├── arch.rs
│   │   │
│   │   ├── linux/
│   │   │   ├── mod.rs
│   │   │   ├── memory.rs
│   │   │   ├── netpoll.rs
│   │   │   └── thread.rs
│   │   │
│   │   ├── macos/
│   │   │   └── mod.rs
│   │   │
│   │   └── windows/
│   │       └── mod.rs
│   │
│   └── metrics/
│       ├── mod.rs
│       ├── runtime_stats.rs
│       ├── scheduler_stats.rs
│       ├── memory_stats.rs
│       └── stack_stats.rs
│
├── examples/
│   ├── basic.rs
│   ├── many_routines.rs
│   ├── channels.rs
│   ├── synchronization.rs
│   ├── timers.rs
│   └── networking.rs
│
├── tests/
│   ├── scheduler_tests.rs
│   ├── routine_tests.rs
│   ├── channel_tests.rs
│   ├── synchronization_tests.rs
│   ├── stack_tests.rs
│   ├── context_tests.rs
│   └── stress_tests.rs
│
└── benches/
    ├── spawn.rs
    ├── scheduler.rs
    ├── context_switch.rs
    ├── channels.rs
    ├── memory.rs
    └── throughput.rs

Responsabilidad de cada directorio
Directorio	Responsabilidad
runtime	Ciclo de vida
scheduler	Planificación
rutina	Unidades de ejecución
processor	Recursos P
machine	Threads M
stack	Stacks independientes
context	Cambio de contexto
queue	Colas
sync	Sincronización
channel	Comunicación
timer	Temporizadores
netpoll	I/O
memory	Administración de memoria
ffi	Fronteras nativas
platform	Adaptación por sistema operativo
metrics	Estadísticas
tests	Validación
benches	Rendimiento
docs	Documentación

No es obligatorio crear todos los archivos inmediatamente. La estructura representa la arquitectura objetivo.

Crear los módulos progresivamente y evitar archivos vacíos que aparenten funcionalidades inexistentes.

PARTE XI — CONFIGURACIÓN DE CARGO
24. Cargo.toml

Crear inicialmente:

toml
[package]
name = "rust-gmp-runtime"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[lib]
name = "rust_gmp_runtime"
path = "src/lib.rs"

[dependencies]

[dev-dependencies]

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true

[profile.dev]
opt-level = 1


No agregar dependencias externas.

No añadir herramientas de profiling como crates.

Si una herramienta de desarrollo externa es utilizada manualmente, no debe incorporarse como dependencia del proyecto.

Política de panic

No establecer panic = "abort" como requisito universal.

Evaluar dos configuraciones:

Desarrollo y pruebas: permitir unwinding cuando sea compatible con el diseño.
Distribución: evaluar abort según las garantías y necesidades del runtime.

No permitir unwinding arbitrario entre contextos de ejecución.

PARTE XII — SEGURIDAD DE MEMORIA
25. Rust Safety Model

Este proyecto utilizará operaciones de bajo nivel.

Por tanto, la seguridad de memoria será una prioridad absoluta.

Minimizar unsafe.

No permitir que los módulos de alto nivel manipulen directamente registros de CPU o punteros internos de stacks.

Toda frontera de bajo nivel debe estar encapsulada.

Requisitos

Evitar:

Data races.
Use-after-free.
Double-free.
Dangling pointers.
Stack corruption.
Undefined Behavior.
Deadlocks.
Race conditions.
Liberación prematura de memoria.
Accesos concurrentes incorrectos.
Violaciones de aliasing.
Referencias inválidas.
Documentación obligatoria de unsafe

Cada bloque unsafe debe explicar:

Por qué es necesario.
Qué invariantes exige.
Qué punteros utiliza.
Quién posee la memoria.
Cuándo puede liberarse.
Qué alineación necesita.
Qué operaciones están permitidas.
Qué garantías proporciona el llamador.

No utilizar unsafe para evitar resolver problemas de ownership.

Particularidad de las rutinas

Las rutinas pueden migrar entre Machines.

Por tanto:

Sus stacks deben permanecer en direcciones estables.
Los objetos internos deben conservar sus garantías de vida.
No se deben mantener referencias inválidas durante cambios de contexto.
Se debe estudiar el impacto de TLS y estado asociado al thread.
No asumir que una rutina tiene siempre el mismo thread.
Las operaciones dependientes del thread deben estar identificadas.

La migración de una rutina entre threads debe estar cuidadosamente diseñada.

PARTE XIII — OPTIMIZACIÓN
26. Objetivo de consumo reducido

La optimización debe formar parte de la arquitectura desde el comienzo.

Prioridades:

Reducir RAM por rutina.
Reducir asignaciones dinámicas.
Reducir fragmentación.
Evitar copias.
Minimizar context switching.
Reducir contención.
Evitar sincronización global innecesaria.
Optimizar localidad de memoria.
Evitar false sharing.
Reducir operaciones atómicas.
Evitar busy waiting.
Dormir eficientemente cuando no exista trabajo.
Reutilizar estructuras.
Evitar metadatos redundantes.
Métricas obligatorias

Implementar mediciones de:

Rutinas creadas.
Rutinas activas.
Rutinas bloqueadas.
Rutinas terminadas.
Machines activas.
Processors activos.
Tareas por Processor.
Longitud de colas.
Cambios de contexto.
Tiempo de espera.
Tiempo de planificación.
Memoria reservada.
Memoria comprometida.
Memoria residente cuando pueda medirse.
Asignaciones.
Liberaciones.
Consumo de CPU.
Throughput.

Diferenciar claramente memoria virtual reservada de RAM residente.

No confundir ambas métricas.

No realizar optimizaciones prematuras

Primero implementar correctamente.

Después medir.

Finalmente optimizar los cuellos de botella reales.

PARTE XIV — FASES DE DESARROLLO
27. Metodología obligatoria

El desglose completo, dependencias, entregables, pruebas y puertas de avance están en `PHASES.md`, que es la fuente de verdad del orden y alcance de fases. Esta especificación conserva el resumen original de alto nivel; ante diferencias de secuencia o entregables, seguir `PHASES.md` sin alterar los requisitos técnicos aquí definidos.

El desarrollo será incremental.

No implementar todos los subsistemas simultáneamente.

No generar el proyecto completo en una única respuesta.

Cada fase deberá terminar con código verificable, pruebas y documentación.

FASE 1 — Arquitectura y fundamentos

Objetivos:

Definir G-M-P.
Definir las estructuras.
Definir ownership.
Definir estados.
Definir transiciones.
Definir interfaces internas.
Definir invariantes.
Definir plataformas.
Definir ABI.
Definir estrategia de stacks.
Definir estrategia de context switching.

Entregables:

Arquitectura documentada.
Diagrama de componentes.
Modelo de estados.
Decisiones técnicas.
Plan de pruebas.
Riesgos identificados.

No implementar todavía el scheduler completo.

FASE 2 — Rutinas y gestión de stacks

Objetivos:

Implementar identificadores.
Implementar estados.
Implementar estructuras de rutina.
Implementar reserva de stacks.
Implementar protección.
Implementar liberación.
Implementar reutilización segura.

Entregables:

Gestor de rutinas.
Stack Manager.
Pruebas de memoria.
Documentación de seguridad.
FASE 3 — Context Switching

Objetivos:

Implementar contexto de CPU.
Implementar Assembly.
Implementar cambio de stack.
Implementar restauración.
Implementar x86_64.
Validar alineación.
Validar registros.
Validar retorno.

Primera plataforma: Linux x86_64.

Entregables:

Motor de context switching funcional.
Pruebas de cambio de contexto.
Documentación de ABI.
Documentación de unsafe.
Benchmarks iniciales.

No avanzar si existen problemas de integridad de stack o registros.

FASE 4 — Machine y Processor

Objetivos:

Crear workers.
Crear Machines.
Crear Processors.
Implementar asociación M-P.
Implementar ciclo de ejecución.
Implementar estados.

Entregables:

Machine Manager.
Processor Manager.
Pruebas de ejecución concurrente.
FASE 5 — Scheduler

Objetivos:

Implementar Scheduler.
Local Run Queue.
Global Run Queue.
Distribución de tareas.
Work Stealing.
Parking.
Unparking.
Balanceo de carga.
Administración de processors.

Entregables:

Scheduler funcional.
Pruebas de concurrencia.
Pruebas de carga.
Benchmarks.
FASE 6 — Suspensión y preemption

Objetivos:

Implementar Yield.
Implementar Waiting.
Implementar Wake.
Implementar Parking.
Implementar puntos seguros.
Implementar preemption cooperativa.
Investigar preemption asíncrona.

Entregables:

Rutinas suspendibles.
Rutinas reanudables.
Pruebas de fairness.
Documentación de limitaciones.
FASE 7 — Sincronización y canales

Objetivos:

Mutex.
RWMutex.
Semaphore.
WaitGroup.
Condvar.
Channels buffered.
Channels unbuffered.
Send.
Receive.
Close.
Select.

Entregables:

Sistema de sincronización funcional.
Sistema de canales funcional.
Pruebas de concurrencia.
FASE 8 — Timers

Objetivos:

Timer Manager.
Sleep.
Timeout.
Deadline.
Cancelación.
Timers periódicos.
Despertar.

Entregables:

Timer System.
Pruebas de temporización.
Benchmarks.
FASE 9 — Netpoller

Objetivos:

Diseñar la abstracción.
Implementar Linux epoll.
Implementar registro de descriptores.
Implementar espera de eventos.
Despertar rutinas.
Integrar timers.

Entregables:

Netpoller Linux funcional.
Pruebas de I/O.
Documentación de FFI.

Después incorporar macOS y Windows.

FASE 10 — Memory Manager

Objetivos:

Slab.
Arena.
Object Pools.
Stack Pools.
Métricas.
Reducción de asignaciones.
Reducción de fragmentación.

Entregables:

Memory Manager.
Pruebas.
Comparativas de memoria.
FASE 11 — Optimización

Objetivos:

Optimizar scheduler.
Optimizar queues.
Reducir contención.
Optimizar context switching.
Optimizar memoria.
Reducir consumo de CPU.
Mejorar balanceo.

Entregables:

Benchmarks.
Resultados reproducibles.
Comparativas.
Informe de optimización.
FASE 12 — Estabilización

Objetivos:

Revisar invariantes.
Revisar código unsafe.
Revisar FFI.
Revisar ciclo de vida.
Revisar errores.
Ejecutar stress tests.
Revisar portabilidad.
Documentar limitaciones.

Entregables:

Versión estable.
Documentación completa.
API pública.
Ejemplos.
Informe de rendimiento.
PARTE XV — TESTING Y VALIDACIÓN
28. Pruebas obligatorias

Cada subsistema debe tener pruebas.

Implementar:

Unit Tests.
Integration Tests.
Stress Tests.
Concurrency Tests.
Memory Tests.
Scheduler Tests.
Stack Tests.
Context Switching Tests.
Channel Tests.
Synchronization Tests.
Timer Tests.
I/O Tests.
Escenarios de carga

Probar:

Rutinas	Objetivo
1.000	Validación inicial
10.000	Escalabilidad
100.000	Carga elevada
500.000	Escalabilidad de stacks y mapas, si los recursos lo permiten
1.000.000	Estrés de memoria, si los recursos lo permiten

No afirmar que el runtime soporta un millón de rutinas únicamente porque permite crear un millón de estructuras.

Deben ejecutarse y comprobarse correctamente.

Escenarios concurrentes

Probar:

Creación simultánea.
Finalización simultánea.
Bloqueo.
Desbloqueo.
Work Stealing.
Contención.
Cierre de canales.
Cancelación.
Apagado del runtime.
Operaciones durante shutdown.
Errores y pánicos.
Presión de memoria.

Utilizar herramientas adicionales de análisis únicamente como herramientas externas de desarrollo, nunca como dependencias de ejecución.

Documentar sus resultados y limitaciones.

PARTE XVI — BENCHMARKS
29. Comparación de rendimiento

Comparar nuestro runtime con una línea base de threads nativos de Rust, usando
cargas de trabajo equivalentes y el mismo límite de concurrencia.

Métricas
Tiempo de spawn.
Tiempo de ejecución.
Latencia de planificación.
Context switching.
RAM por tarea.
RAM total.
Memoria residente.
Memoria virtual.
CPU.
Throughput.
Escalabilidad.
Tiempo de espera.
Contención.
Escenarios

A. Tareas pequeñas de CPU.

B. Tareas de espera.

C. Comunicación mediante canales.

D. Carga mixta.

E. Miles de tareas simultáneas.

F. Concurrencia de red.

G. Carga sostenida.

No comparar implementaciones con semánticas diferentes sin explicar las diferencias.

No inventar resultados.

No afirmar mejoras de rendimiento sin evidencia reproducible.

Todos los benchmarks deben ser reproducibles.

PARTE XVII — API PÚBLICA
30. API final deseada

La API debe ser sencilla.

Ejemplo:

rust
use rust_gmp_runtime::Runtime;

fn main() {
    let runtime = Runtime::builder()
        .processors(4)
        .build();

    for i in 0..10_000 {
        runtime.spawn(move || {
            println!("Rutina {}", i);
        });
    }

    runtime.wait();
}


API objetivo:

rust
Runtime::new()
Runtime::builder()
Runtime::spawn()
Runtime::wait()
Runtime::shutdown()

spawn()
yield_now()
sleep()
timeout()

channel()
send()
recv()
select()

mutex()
rwmutex()
semaphore()
wait_group()


Las firmas definitivas deben adaptarse al sistema de ownership de Rust.

Utilizar restricciones Send y 'static cuando correspondan.

No prometer que cualquier función bloqueante de la biblioteca estándar puede suspender automáticamente una rutina.

Las operaciones administradas por nuestro runtime deberán utilizar las primitivas del propio runtime.

Las operaciones externas bloqueantes deberán documentarse y gestionarse mediante una estrategia apropiada de syscall, threads auxiliares o adaptadores específicos.

PARTE XVIII — REGLAS DE DESARROLLO PARA EL AGENTE
31. Normas obligatorias
No implementar todo de golpe.
No crear un Thread Pool convencional.
No sustituir las rutinas por tareas async.
No utilizar Tokio.
No introducir crates externos.
No inventar APIs de Rust.
No inventar APIs de sistemas operativos.
No utilizar Assembly sin justificarlo.
No utilizar FFI innecesariamente.
No utilizar unsafe sin documentación.
No ignorar las garantías de ownership.
No mover stacks activos arbitrariamente.
No permitir corrupción de contexto.
No dejar código inseguro sin pruebas.
No crear interfaces vacías que simulen funcionalidades.
No declarar compatibilidad multiplataforma sin pruebas.
No realizar optimizaciones sin mediciones.
No duplicar responsabilidades entre módulos.
No crear archivos gigantescos.
No ocultar errores de implementación.
32. Procedimiento obligatorio por cada módulo

Antes de escribir código:

Explicar la responsabilidad.
Explicar el algoritmo.
Definir las estructuras.
Definir las interfaces.
Explicar ownership.
Explicar sincronización.
Explicar consumo de memoria.
Identificar riesgos.
Definir pruebas.

Después:

Implementar.
Compilar.
Ejecutar pruebas.
Corregir errores.
Revisar código unsafe.
Documentar.
Medir cuando corresponda.

No avanzar a un módulo dependiente si el anterior no cumple sus criterios de aceptación.

33. Informe obligatorio al finalizar cada fase

Entregar siempre:

text
PHASE REPORT

1. Phase:
2. Objectives:
3. Architecture:
4. Files created:
5. Files modified:
6. Features implemented:
7. Tests executed:
8. Test results:
9. Memory considerations:
10. Performance considerations:
11. Safety review:
12. Known limitations:
13. Pending work:
14. Next phase:


No presentar como completado ningún componente que solamente tenga código parcial.

PARTE XIX — CRITERIOS DE ACEPTACIÓN FINAL
34. Definición de runtime terminado

El proyecto podrá considerarse un runtime funcional cuando cumpla todas estas condiciones:

Ejecución
Ejecuta rutinas stackful.
Cada rutina mantiene su propio contexto.
Las rutinas pueden suspenderse.
Las rutinas pueden reanudarse.
Las rutinas pueden migrar entre Machines de forma segura.
Scheduler
Implementa G-M-P.
Implementa Local Run Queues.
Implementa Global Run Queue.
Implementa Work Stealing.
Implementa balanceo.
Implementa parking y unparking.
Memoria
Administra stacks independientes.
Evita liberaciones prematuras.
Protege stacks.
Mantiene ownership.
Controla las asignaciones.
Mide el consumo real.
Concurrencia
Implementa Channels.
Implementa Select.
Implementa Mutex.
Implementa WaitGroup.
Implementa Semaphore.
Gestiona rutinas bloqueadas.
Sistemas
Implementa timers.
Implementa Netpoller.
Integra APIs nativas.
Mantiene una arquitectura multiplataforma.
Calidad
Supera pruebas de estrés.
Supera pruebas de concurrencia.
No presenta corrupción de stacks.
No presenta data races conocidas.
Documenta todos los límites de seguridad.
Dispone de benchmarks reproducibles.

PARTE XX — INSTRUCCIÓN INICIAL
35. Comenzar el proyecto

A partir de este documento, debes actuar como arquitecto principal y responsable de implementación del Rust GMP Runtime.

No comiences generando todos los archivos.

No implementes el scheduler antes de definir correctamente la arquitectura de ejecución.

Comienza exclusivamente por la FASE 1.

Primera entrega obligatoria

Debes producir:

Documento de arquitectura general.
Diagrama G-M-P.
Diseño del ciclo de vida de una rutina.
Diseño de las estructuras G, M y P.
Diseño de las colas.
Modelo de ownership.
Modelo de estados.
Estrategia de Stack Management.
Estrategia de Context Switching.
Diseño de la frontera Rust / Assembly / FFI.
Análisis de seguridad.
Estrategia de memoria.
Interfaces internas.
Arquitectura de carpetas.
Plan de pruebas.
Registro de decisiones arquitectónicas (ADR).
Restricción de avance

No pasar a la FASE 2 hasta que la FASE 1 esté completamente documentada y revisada.

Si encuentras una limitación técnica, no la ocultes ni inventes una solución.

Explica:

Qué limita la implementación.
Por qué existe la limitación.
Qué alternativas existen.
Qué coste tiene cada alternativa.
Qué solución recomiendas.
Cómo se validará.

No reemplazar una funcionalidad por una simulación sin autorización.

No modificar los objetivos fundamentales del proyecto.

OBJETIVO FINAL: CONSTRUIR UN RUNTIME M:N DE RUTINAS STACKFUL, IMPLEMENTADO NATIVAMENTE EN RUST, CON SCHEDULER G-M-P PROPIO, CONTEXT SWITCHING PROPIO, GESTIÓN DE STACKS, SINCRONIZACIÓN Y ADMINISTRACIÓN DE RECURSOS, SIN DEPENDENCIAS EXTERNAS.

El resultado debe ser un runtime de sistemas real, no una abstracción sobre librerías existentes.

2. Decisiones técnicas que debes conservar
Hay cinco puntos del documento que son especialmente importantes y que no conviene modificar durante el desarrollo:

Decisión

Motivo

G-M-P

Es el modelo central de planificación

Stacks de direcciones estables

Evita invalidar referencias de Rust

Assembly + FFI aislados

Permite controlar el motor de ejecución

Linux x86_64 primero

Reduce complejidad inicial

Cero crates externos

Garantiza que el runtime sea nuestro

3. Tres advertencias técnicas para el desarrollo
Quiero dejarte tres aspectos que el agente debe tener presentes desde el principio:

Stackful no significa automáticamente compatible con cualquier código Rust. Hay que estudiar TLS, destructores, pánicos, referencias, operaciones dependientes del thread y la ABI.

Una función síncrona puede bloquear el thread. Nuestro runtime podrá suspender las rutinas que utilicen sus propias primitivas, pero no puede interceptar mágicamente cualquier llamada bloqueante de std.

La preemption de rutinas stackful es compleja. Primero se debe conseguir una planificación cooperativa correcta y después investigar mecanismos de preemption más avanzados.

Se recomienda mantener este documento como la especificación oficial del proyecto y trabajar fase por fase. Así evitamos generar miles de líneas de código aparentemente funcionales sin haber resuelto primero el problema más importante: el motor de ejecución y la seguridad de los stacks.

PARTE XXI — RESULTADOS Y REQUISITOS DE LA AUDITORÍA TÉCNICA

36. Alcance y estado del proyecto

Esta especificación define objetivos; no prueba que exista una implementación. El estado se determina exclusivamente por código, pruebas y resultados reproducibles. El scaffold inicial no constituye un runtime funcional.

La auditoría y el diseño de FASE 1 no autorizan a implementar el runtime. Antes de avanzar, entregar los documentos requeridos y resolver o registrar con alternativas, costes y método de validación todos los riesgos críticos.

37. TLS, afinidad y migración de rutinas

El runtime no virtualiza ni controla automáticamente el TLS de Rust, del sistema operativo o de bibliotecas nativas. `thread_local!` y otros datos TLS pertenecen al thread M: una rutina migrada puede observar el TLS de otra M y los destructores TLS se ejecutan según el ciclo de vida del thread, no el de la rutina.

La API de rutinas migrables no prometerá semántica de thread-local por rutina. La compatibilidad con código síncrono Rust se limita a código que tolera migración y el comportamiento real del TLS del thread. No existe una detección general que permita al runtime descubrir que una clausura arbitraria retiene estado TLS o recursos ligados al thread.

**Decisión F1:** G será migrable por defecto. La API inicial no ofrece
ejecución fijada a M ni task-local propio. El usuario debe evitar asumir
persistencia de TLS entre yields; el runtime no detecta esa dependencia en una
closure. `Send`, `!Send` y `#[inline(never)]` no prueban afinidad. Recursos que
requieren un thread deberán rechazarse o esperar una API de afinidad futura con
ADR. F4/F12 comprobarán ejemplos TLS/migración, lo que no equivale a garantía
universal de compatibilidad.

38. Política de panic, resultado y recursos

Toda G empieza en un punto de entrada controlado. Con unwinding habilitado, `catch_unwind` debe envolver el cuerpo de la G y capturar el panic en su propio stack antes de retornar al scheduler. Ningún unwind puede atravesar el límite de cambio de contexto ni frames de Assembly sin soporte formal explícito.

**Decisión F1:** el trampoline de G produce éxito, panic capturado o error de
runtime observable por handle/scheduler. Con `panic = "unwind"`, `catch_unwind`
envuelve el cuerpo en el stack de G y captura antes del retorno normal; el
unwind nunca cruza context switch/Assembly. Otras G continúan tras un panic. El
payload tiene ownership explícito en el resultado o se descarta al destruirlo.
Un doble panic/destructor que paniquea puede abortar. Con `panic = "abort"` el
proceso termina; no se impondrá ese perfil a dependientes. Esto no es garantía
hasta probar ABI/unwinder en F3/F12.

39. Syscalls y frontera de plataforma

Usar APIs de `std` donde sean suficientes, particularmente para creación/administración de threads y sincronización. No crear threads con syscalls crudas. Reservar FFI/syscalls para funciones sin alternativa adecuada en `std`, y revisar cada llamada de forma independiente.

**Decisión F1:** usar `std::thread` para crear M y `std::sync`/parking de `std`
cuando baste; no hacer syscalls crudas para crear threads. No usar ABI kernel
directa inicialmente. Stacks protegidos pueden requerir FFI privada a API de
memoria; F2 debe verificar ABI libc, firma, tipos, sentinels, errores y
parámetros contra documentación/header del target antes de implementar. `epoll`
se trata en F9 con verificación equivalente. `futex`, señales y preemption
asíncrona quedan fuera mientras `std` cumpla. La lista `mmap`, `mprotect`,
`munmap`, `epoll_*`, `futex` no obliga a incluirlas todas.

La ausencia de crates externos no significa que la implementación interna de `std` carezca de dependencias del sistema. El alcance es no añadir crates/dependencias externas al proyecto y aislar/documentar las dependencias nativas realmente utilizadas.

40. Stacks, guard pages y overflow

Evaluar reserva virtual con compromiso bajo demanda, pero no equiparar páginas residentes iniciales, páginas comprometidas y espacio reservado. Los valores experimentales de 8 KiB comprometidos inicialmente y 256 KiB reservados son únicamente puntos de benchmark dependientes de plataforma, tamaño de página, compilador y carga.

**Decisión F1:** stack de capacidad fija, acotada y de dirección estable; no
crecimiento automático, copy/relocation, ni reutilización en la primera
implementación. Overflow de la guardia es fail-stop, no recuperación/panic de G.
El tamaño no se fija con las hipótesis 8/256 KiB; F2 lo mide antes de estabilizar
un default. La reserva individual es baseline inicial de corrección; F2/F10
comparan slabs por syscalls y VMA, pues `mprotect` puede dividir VMAs. No
afirmar escala de cientos de miles/millón sin pruebas. Probar `println!`,
recursión, locales grandes, fallos y overflow fatal en procesos aislados.

41. Colas y sincronización: baseline correcto antes de lock-free

Las colas local, global y de work stealing empiezan con una implementación convencional protegida por `Mutex` u otro mecanismo de `std` justificado. Los bloqueos de metadatos/colas deben tener secciones críticas acotadas y nunca atravesar yield, suspensión o I/O. Probar primero concurrencia y estrés; medir latencia y contención; optimizar solo al identificar un cuello de botella.

Cualquier algoritmo lock-free posterior requiere ADR y revisión formal de ownership, invariantes, órdenes de memoria, ABA, reclamación/liberación y carreras, junto con pruebas y benchmarks. Una prueba de estrés no demuestra por sí sola ausencia de errores de concurrencia.

42. Netpoller y operaciones de I/O

La API interna debe expresar modelos de readiness y completion por separado. Readiness (Linux `epoll`, macOS `kqueue`) informa disponibilidad y puede requerir reintento de la operación; completion (Windows IOCP) informa el resultado de una operación ya iniciada. Definir registros/operaciones, IDs, asociación a G, cancelación, errores, finalización, wake-up y ciclo de vida sin fingir que estos modelos son intercambiables.

**Decisión F1:** primer backend previsto es readiness Linux `epoll`, separado
de completion Windows IOCP. Sockets no bloqueantes y pipes se validarán
separadamente. Archivos regulares quedan fuera del Netpoller inicial; llamadas
`std::fs` arbitrarias pueden bloquear M y no son interceptadas. Solo I/O
explícito del runtime puede hacer handoff de P. Offload de archivos requerirá
ADR para workers acotados, cola/backpressure, cancelación y shutdown. F9 probará
los tipos de I/O incluidos en soporte; no se afirma I/O general.

43. Riesgos de arquitectura identificados y respuesta

- **Crítico — Integridad de contexto y unwinding:** captura de panic o guard pages no garantizan la seguridad del stack alternativo. Resolver contratos de ABI, trampoline, stack y unwind antes de implementar ejecución.
- **Crítico — TLS y afinidad:** una G migrable no conserva TLS del thread. Limitar explícitamente compatibilidad; diseñar pinning/task-local si se necesita, sin prometer detección automática.
- **Alto — Syscalls/FFI:** ABI libc y ABI de kernel pueden diferir y syscalls de creación de threads evitan inicialización necesaria de Rust. Preferir `std`; cada excepción requiere fuente y contrato revisados.
- **Alto — I/O y plataformas:** readiness/completion no son equivalentes y archivos regulares no son universalmente multiplexables. Separar modelos y planificar offload acotado.
- **Alto — Stack overflow/growth:** reserva virtual y guard page no prueban crecimiento/recuperación segura. No declarar growth antes de implementación y pruebas específicas.
- **Medio — Colas lock-free prematuras:** objetivos de bajo overhead no justifican complejidad sin mediciones. Empezar con estructuras sincronizadas y optimizar evidencia.
- **Medio — Toolchain:** `Cargo.toml` declara Rust 1.85 y `rust-toolchain.toml` fija 1.85.0; FASE 1 debe validar esa versión para el código real y revisar cada API/Assembly futura contra el MSRV.
- **Medio — Diferencia entre plan y estado:** placeholders, carpetas y ejemplos vacíos no acreditan funciones; la aceptación requiere ejecución y pruebas del runtime real.

44. Documentación técnica de FASE 1

Los entregables documentales de F1 con contenido técnico y decisiones explícitas
son:

- `docs/architecture.md`, `docs/scheduler.md`, `docs/memory-model.md`, `docs/context-switching.md`, `docs/stack-management.md`.
- `docs/panic-and-unwinding.md`, `docs/tls-and-thread-affinity.md`, `docs/syscalls.md`, `docs/netpoller.md`, `docs/io-strategy.md`.
- `docs/synchronization.md`, `docs/platform-support.md`, `docs/performance.md`, `docs/security-invariants.md`.

Cada ADR y decisión sustantiva debe registrar problema, alternativas, decisión, justificación, coste de CPU, coste de memoria, riesgos, limitaciones y validación. Mantener el mapa de carpetas como arquitectura objetivo y crear cada módulo de código solo en su fase y cuando tenga responsabilidad concreta.

La fase registra asimismo su diagnóstico y evidencia en
`docs/phases/phase-1-report.md`. El informe distingue diseño acordado de
comportamiento probado.

45. Pruebas adicionales obligatorias

Agregar casos planificados antes de implementar y ejecutar cuando exista el componente:

- Panic dentro de G, captura/resultado, retorno correcto al scheduler, liberación de recursos, continuidad según política y builds `unwind`/`abort`.
- TLS observado por M, migración y recursos/thread affinity; demostrar límites sin asumir que una prueba aislada garantiza compatibilidad general.
- Stacks pequeños, overflow/guard page, reutilización/liberación, recursión, locals grandes y formatos/`println!`.
- Scheduler: handoff y recuperación de P, bloqueo/wake-up, Work Stealing, alta concurrencia y ausencia de ejecución simultánea de G.
- I/O real con sockets, pipes y archivos regulares; bloqueos, cancelaciones y errores nativos.
- Sincronización: múltiples waiters, wake-up, carreras, deadlock y recursos liberados; combinar pruebas dirigidas con stress tests y análisis adicional disponible.

La matriz de aceptación de FASE 12 y `PHASES.md` deberán incorporar todos estos requisitos. Ningún resultado hipotético se presentará como validación realizada.

PARTE XXII — ORGANIZACIÓN, MSRV, ESCALABILIDAD DE STACKS Y API FUTURA

46. Documentos normativos y uso por tarea

`SPEC.md` es la fuente de verdad técnica permanente. `PHASES.md` es la fuente de verdad para secuencia, alcance, módulos, pruebas y criterios de avance. Los documentos de `docs/` contienen decisiones y detalle de subsistemas sin redefinir estos contratos.

En cada tarea futura, proporcionar `SPEC.md`, únicamente la fase de `PHASES.md` que se solicitó, el código existente y las instrucciones específicas. El agente trabaja exclusivamente en esa fase; no implementa trabajo futuro anticipadamente. Si los documentos divergen, mantener los requisitos técnicos de `SPEC.md` y actualizar `PHASES.md` de forma explícita antes de cambiar el alcance.

47. Cargo.lock, dependencias y MSRV

El proyecto es inicialmente una biblioteca sin dependencias externas. Mantener `[dependencies]` y `[dev-dependencies]` vacíos. No versionar un `Cargo.lock` generado únicamente por esta biblioteca; Cargo puede crearlo localmente al compilar. Reevaluar su inclusión si se añade una aplicación, binario o workspace de pruebas que justifique reproducibilidad a nivel de producto.

Contrato inicial del paquete:

```

Las APIs de la lista son objetivos por fase, no un contrato público ya
implementado. La API inicial usará ownership seguro; `spawn` requiere
`Send + 'static` para closures/valores cuando corresponda y las tareas son
migrables por defecto. Se definirá un handle que observe éxito/panic/fallo y un
shutdown que cierre admisión y drene el trabajo aceptado. No exponer `scope`,
pinning ni task-local hasta tener ADR y garantías de lifetime/afinidad. Las
llamadas síncronas arbitrarias del usuario pueden bloquear M; solo operaciones
explícitas del runtime pueden suspender G y hacer handoff de P.

La API concreta, tipos de error y semántica exacta de `wait`/`shutdown` se
estabilizan cuando se implemente Runtime en F4; esta sección no autoriza a
adelantar esa fase.
[package]
name = "rust-gmp-runtime"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[lib]
name = "rust_gmp_runtime"
path = "src/lib.rs"

[dependencies]

[dev-dependencies]
```

Licencia dual: MIT o Apache-2.0. Titular indicado en el aviso de copyright MIT: KEVINS YESID VEGA MARMOLEJO.

La toolchain inicial se fija en Rust 1.85.0 para validar el MSRV declarado. Todas las funcionalidades utilizadas deben compilar en Rust 1.85 estable. Verificar especialmente `asm!`, `global_asm!`, `extern`, sintaxis y ABI FFI, operaciones atómicas, APIs de `std`, atributos y opciones de compilación. No usar features inestables sin autorización expresa. Si una capacidad necesaria no existe en 1.85 estable, registrar evidencia, alternativa y coste antes de modificar `rust-version`.

El contrato de compatibilidad se prueba con el toolchain 1.85 para el código real existente y se mantiene en CI. Compilar el scaffold no valida APIs/Assembly que aún no se hayan implementado.

48. Slabs de stacks y límites de mapeos Linux

El coste de stack a gran escala incluye memoria virtual, páginas residentes/comprometidas, metadatos, llamadas de mapeo, fragmentación y VMA count. En Linux, muchos mapeos o cambios de protección pueden incrementar las áreas de memoria del proceso y alcanzar `vm.max_map_count` antes de agotar RAM.

El Stack Manager debe comparar explícitamente:

- **Mapeo por stack:** reserva y protección independientes; aislamiento simple y liberación individual, pero posibles mapeos/VMA y syscall overhead por stack.
- **Slab de espacio virtual:** reserva de regiones grandes y subdivisión en slots de stack. Medir crecimiento de slabs, alineación, bitmap/free list, fragmentación, política de liberación y guard regions. Agrupar reservas puede reducir syscalls, pero `mprotect` en cada slot puede dividir VMA; no asumir que un slab elimina la presión de `vm.max_map_count`.
- **Pool de stacks terminados:** reutilización solo después de comprobar que G terminó, no quedan operaciones/referencias, el stack fue limpiado o reinicializado según el contrato y sus guard/protection state volvió a un estado conocido. Establecer límites de retención y devolver slabs al SO cuando el runtime ya no los necesita.

No asumir que cada guard page requiere un `mmap` independiente ni que una disposición de slab evita VMAs. Elegir topología tras medir `mmap`/`mprotect`, `/proc/self/maps`, uso base y crecimiento del número de VMA en la plataforma. Documentar el límite observado de `vm.max_map_count`, el espacio virtual/límites de proceso relevantes y el entorno de prueba. No cambiar parámetros globales del SO automáticamente ni para benchmarks ni para ejecución.

49. Métricas y pruebas de escalabilidad de stacks

El Stack Manager/metrics debe poder registrar:

- Número total, activos, en espera, reutilizados y liberados de stacks.
- Número de slabs y capacidad libre/ocupada por slab.
- Mapeos/VMA observados cuando la plataforma permita medirlos.
- Memoria virtual reservada, páginas comprometidas/tocadas y memoria residente (RSS) por separado.
- Fragmentación interna/externa observable, overhead de metadatos y bytes por G.
- Asignaciones, liberaciones, reutilización, fallos y latencia de reserva/liberación.
- Tiempo de creación, planificación y finalización; CPU y RAM del workload.

Planificar cargas progresivas de 1.000, 10.000, 100.000, 500.000 y 1.000.000 de rutinas, únicamente si recursos y configuración existente lo permiten. Distinguir instancias G meramente creadas de rutinas ejecutadas, bloqueadas y completadas; no afirmar capacidad de runtime basada solo en estructuras asignadas.

Registrar tamaño real probado, RAM, CPU, mapeos/VMA, tiempo de creación/scheduling/finalización y errores de asignación. Reportar límites ambientales, incluyendo `vm.max_map_count` observado, sin modificarlo automáticamente. Presión de memoria, fallos y tiempos deben tratarse como resultados, no ocultarse.

50. API inicial y evaluación futura de scopes

La API inicial de `spawn` conservará las restricciones de ownership necesarias, incluidas `Send` y `'static` cuando correspondan. Una tarea debe poder completar antes de que su estado/stack se libere; el contrato de `wait`/shutdown y resultado debe ser explícito.

Evaluar `Runtime::scope`/scope de tareas prestadas como mejora futura inspirada en `std::thread::scope`, no como parte de la implementación inicial. No implementarla hasta demostrar cómo la salida del scope espera o une todas las rutinas hijas y cómo garantiza que ninguna referencia prestada, operación pendiente ni stack sobreviva al lifetime del scope. Diseñar comportamiento ante panic, cancelación, spawn anidado, deadlock y bloqueos antes de exponer API.

La seguridad de lifetime prima sobre comodidad. Si no puede expresarse y probarse el contrato seguro en Rust estable, mantener `scope` fuera de la API; no compensar con `unsafe` no probado.