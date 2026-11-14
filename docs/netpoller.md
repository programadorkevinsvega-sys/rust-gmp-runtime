# Arquitectura del Netpoller

## Contrato común mínimo, semánticas separadas

- **Readiness:** `epoll` (Linux) y `kqueue` (macOS) notifican disponibilidad
  potencial. El runtime aún intenta la operación y trata `WouldBlock`.
- **Completion:** IOCP (Windows) informa el resultado de una operación ya
  iniciada, que tiene identidad y ciclo de vida propios.

Compartir solo identidad, asociación con G, cancelación, errores, wake-up y
shutdown cuando sus contratos coincidan. El tipo de evento/resultado y el
ownership de la operación seguirán siendo específicos del backend; no exponer
una supuesta finalización para un evento de readiness.

## Decisión de primera plataforma y ciclo de vida

La primera integración, si se autoriza al llegar F9, será Linux x86_64 readiness
mediante `epoll`. La API pública no afirmará I/O asíncrono genérico. Registrar
operación -> pending -> readiness -> intentar operación -> resultado o
`WouldBlock`/rearmado -> wake-up exactamente una vez -> desregistro/liberación.
La G/descriptor permanece viva mientras el backend pueda devolver eventos.
Cancelación/shutdown invalidan la generación de registro antes de reciclar IDs,
y esperan que el backend ya no pueda referenciar los recursos.

## Tipos de I/O y límites

Sockets y pipes se validarán por separado con descriptores no bloqueantes.
Archivos regulares no se registran en `epoll` y quedan fuera del Netpoller
inicial. Las operaciones regulares de `std::fs` invocadas por la tarea siguen
siendo síncronas y pueden bloquear M; el runtime no las intercepta
automáticamente. Un futuro offload requiere cola y cantidad de workers
auxiliares acotadas, backpressure, ownership/cancelación y shutdown aprobados en
ADR. No crear thread por operación ni workers ilimitados.

## Handoff y pruebas

Una espera gestionada suspende G y hace handoff de P; la llegada de readiness
no equivale a resultado ni despierta dos veces. Probar I/O real para sockets y
pipes, además de casos de archivo regular que demuestren la limitación o futura
estrategia de offload. Cubrir registro, deregistro, eventos duplicados,
cancelación, cierre, timeout, errores y shutdown. Cada plataforma requiere
build y pruebas propios.

**Estado:** contrato y límites de la primera plataforma definidos; no hay
Netpoller implementado.
