# Pruebas, fuzzing y concurrencia

- R-401 Toda implementación incluye pruebas unitarias, de integración y de
  error (cada variante alcanzada), además de pruebas de concurrencia cuando
  aplique.
- R-402 Fuzzing en dos niveles:
  - En el crate y con `std` únicamente: pruebas aleatorias con semilla fija y
    reproducible, generador propio (xorshift) y secuencias de operaciones
    comparadas con un modelo de referencia (por ejemplo, cola local frente a
    `VecDeque`, timer wheel frente a `BinaryHeap`, canal frente a modelo
    secuencial).
  - Externo: `cargo-fuzz` en `fuzz/`, como crate separado y fuera del paquete
    publicado, solo como herramienta de desarrollo. Cada hallazgo se convierte
    en una prueba de regresión.
- R-403 Repetir las pruebas de estrés de concurrencia; verificar orderings con
  herramientas externas (Loom, Miri, ThreadSanitizer) según aplique, nunca como
  dependencias del runtime.
- R-404 Cobertura mínima: todo camino de error y toda transición de estado.
- R-405 Un bug corregido exige primero una prueba que lo reproduzca.
