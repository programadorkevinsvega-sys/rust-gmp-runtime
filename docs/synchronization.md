# Synchronization

## Decisiones de implementación

La cola inicial del scheduler y el estado compartido usarán primitivas
convencionales de `std`, comenzando con `Mutex` donde haya mutación compartida.
No introducir algoritmos lock-free ni spinlocks como mecanismo predeterminado.
Medir contención antes de optimizar.

Las primitivas propias integradas con el scheduler deben suspender G y liberar P
cuando esperan. `std::sync::Mutex` bloquea el thread que lo toma y no es
automáticamente una primitiva cooperativa de rutina. Mantener críticos
cortos, no ceder ni hacer I/O bajo lock, y documentar cómo se despiertan
waiters. No prometer fairness FIFO hasta fijar y probar una política.

Los contratos antes de implementar cada primitiva deben definir orden de
espera, fairness, poison/error, cancelación, cierre/shutdown, wake-up único y
destrucción de waiters para Mutex, RWMutex, Semaphore, WaitGroup y Condvar.
Definir atomics/memory order solo después de formular invariantes y revisar el
ciclo de vida de los waiters.

## Pruebas planificadas

Probar contención, wake-up de uno/muchos, cancelación, carreras, deadlock,
liberación y shutdown concurrente. Comparar secuencias con modelos secuenciales
de referencia y usar semillas reproducibles. Complementar pruebas dirigidas y
stress; pasar stress no demuestra ausencia de data races o deadlocks.
