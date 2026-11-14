# TLS y afinidad a threads

## Semántica

Una rutina stackful puede ceder en una Machine y reanudarse en otra. El TLS (`thread_local!`, TLS nativo y estado de bibliotecas) pertenece al thread M, no al stack/contexto G. Una G migrada puede observar valores diferentes. Los destructores TLS se asocian al ciclo de vida del thread y no son destructores por rutina.

El runtime no puede inspeccionar en general una closure arbitraria y detectar si accede a TLS o retiene un recurso ligado a un thread. `Send`, `'static` y `#[inline(never)]` no garantizan semántica task-local ni resuelven afinidad.

## Decisión de API inicial

- G es migrable por defecto. El contrato de `spawn` requiere las garantías
  ordinarias de ownership/concurrencia (`Send` y `'static` según corresponda),
  pero no promete persistencia de TLS entre yields.
- No habrá API de G fijada a M ni task-local en la API inicial. Añadirlas
  requiere ADR, ownership/lifecycle, capacidad y shutdown especificados antes de
  implementarlas.
- El usuario es responsable de no depender de TLS o recursos thread-affine a
  través de un yield/migración. El runtime no puede inspeccionar una closure
  arbitraria para detectar esta dependencia.
- Operaciones que requieren el mismo thread deberán ser rechazadas por una API
  futura o ejecutarse mediante una afinidad explícita, si se diseña y prueba.
  No inferir afinidad desde `Send`, `'static`, `!Send` o `#[inline(never)]`.
- Estado por G deberá usar una abstracción propia futura, separada del TLS del
  SO; no prometer virtualización general de TLS.

## Matriz de pruebas y límites

F4.2 comprueba la identidad TLS de cada worker y su `ThreadId`. Cuando el
dispatch y yield permitan migración (F4.3/F6), comprobar valores TLS por M y que
no se presentan como estado task-local; probar recursos thread-affine solo si
se introduce una API de afinidad. Verificar join/shutdown y destructores por
thread. Las pruebas demuestran los casos ensayados, no compatibilidad universal
con cualquier crate o código nativo.

**Estado:** F4.2 registra el `MachineId` en TLS de cada worker y captura su
`ThreadId`; las pruebas confirman que los workers tienen registros y threads
distintos. Todavía no hay ejecución ni migración de G, así que la prueba de
observación de TLS desde una G que migra queda pendiente de las fases de dispatch
y yield. No existe API de pinning: F1 no la aprobó.
