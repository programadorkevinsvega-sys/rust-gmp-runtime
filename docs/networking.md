# Networking

## Alcance del primer backend

El primer backend previsto es readiness-based Linux `epoll` para I/O que el sistema soporte. El objetivo es suspender G mientras espera un evento y dejar P disponible; el evento de readiness no garantiza que una operación posterior haya terminado.

## Clases y limitaciones de operación

- **Sockets:** readiness para sockets no bloqueantes; la operación debe
  reintentarse y puede devolver `WouldBlock`.
- **Pipes:** validar soporte, flags, cierre y wake-up por separado.
- **Archivos regulares:** fuera del Netpoller inicial; no registrarlos en
  `epoll`. Llamadas síncronas de archivos ejecutadas directamente por una G
  pueden bloquear M. El runtime no intercepta esas llamadas.
- Un futuro offload requiere decisión/ADR de workers auxiliares acotados,
  backpressure, propiedad de operación, cancelación, errores y shutdown; no es
  parte de F1 ni una garantía actual.

## Portabilidad

macOS `kqueue` sigue readiness-based; Windows IOCP es completion-based y debe conservar su ciclo de operación/finalización y cancelación. Ver [netpoller](./netpoller.md), [I/O strategy](./io-strategy.md) y [platform support](./platform-support.md). No se declara backend funcional hasta probar I/O real.
