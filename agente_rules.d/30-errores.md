# Errores y fallos

- R-301 Cada módulo define su tipo de error (`enum`) con `Display` e
  `impl std::error::Error`, conservando la causa mediante `source()`.
- R-302 Prohibido `unwrap`, `expect`, `panic!`, `unreachable!` y `todo!` en
  código de producción. Las pruebas devuelven `Result<(), E>` y usan `?`.
- R-303 Syscalls y FFI: comprobar siempre el valor de retorno (`-errno` en
  syscalls crudas, `MAP_FAILED` en `mmap`) y convertirlo en error tipado con la
  operación y los parámetros.
- R-304 Violaciones irrecuperables de invariantes usan una única función
  `fatal(msg)` que registra un mensaje claro y aborta el proceso. Nunca
  continuar en silencio con estado corrupto.
- R-305 Un panic no puede cruzar frames de context switching o Assembly:
  capturarlo con `catch_unwind` en el borde de cada rutina o abortar
  explícitamente.
- R-306 `_` solo es aceptable en patrones donde el compilador lo exige y no
  oculta información (por ejemplo, al desestructurar una tupla). Para
  variables o parámetros sin uso, refactorizar en vez de renombrarlos con `_`.
- R-307 Prohibido `#[allow(...)]`. Excepción única: `#[allow(unsafe_code)]` en
  los módulos context, stack, ffi y platform. Para cualquier otra excepción,
  usar `#[expect(lint, reason = "...")]` con un ADR asociado.
