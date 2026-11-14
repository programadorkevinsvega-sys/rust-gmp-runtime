# `unsafe` y seguridad

- R-701 Cada bloque `unsafe` lleva un comentario `// SAFETY:` con invariantes,
  propietario de la memoria, alineación, condiciones de liberación y garantías
  que aporta el llamador. Cada `unsafe fn` tiene una sección `# Safety`.
- R-702 Un solo `unsafe` por bloque.
- R-703 Solo los módulos context, stack, ffi y platform pueden contener
  `unsafe`. El resto declara `#![forbid(unsafe_code)]`.
- R-704 Cada bloque `unsafe` tiene al menos una prueba; las operaciones de
  memoria se ejecutan bajo Miri cuando sea aplicable.
- R-705 Verificar las firmas FFI y de syscalls contra documentación oficial,
  nunca de memoria. Citar la fuente en el comentario.
