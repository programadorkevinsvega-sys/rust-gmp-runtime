# Syscalls y FFI nativo

## Decisión de frontera

No añadir crates externos no significa evitar toda dependencia del sistema
operativo: `std` utiliza servicios del SO internamente. El objetivo es no añadir
crates externos al proyecto y mantener las interfaces nativas explícitas y
aisladas.

Preferir `std::thread`, `std::sync`, `std::time`, `std::net` y otras APIs
estándar cuando cumplan el requisito. Crear y administrar M con `std::thread`;
no usar `clone` ni syscalls crudas para crear threads. Usar parking/sync de
`std` al comienzo.

La excepción prevista para stacks es reservar/proteger memoria virtual, que no
tiene una interfaz portable adecuada en `std`. F2 evaluará una frontera privada
`extern "C"` contra la interfaz de la plataforma, con ABI libc verificada y
fuente normativa; no se escribirá un wrapper de syscall cruda en la fase
inicial. Si esta interfaz no es viable en el target concreto, detenerse y
registrar alternativa/impacto antes de usar otro ABI. No compilar ni habilitar
`unsafe` hasta aprobar firmas y contratos.

`epoll` se decidirá e implementará en F9 con la misma exigencia de verificación.
`futex`, señales y preemption asíncrona quedan fuera de la arquitectura inicial
mientras `std` cumpla parking; no se implementan por anticipado.

## Registro por llamada

Cada FFI/syscall elegida debe documentar:

1. Necesidad y alternativa de `std` evaluada.
2. Fuente normativa y versión/plataforma.
3. ABI elegida: libc o syscall del kernel; no asumir que coinciden.
4. Firma, tipos, número de syscall si aplica, registros, argumentos, clobbers y convenciones.
5. Retorno, errores, conversión y validación de parámetros (`MAP_FAILED`,
   `-1`/`errno` u otra semántica real de la interfaz; no asumir una común).
6. Ownership, vida útil, seguridad, pruebas y límites.

Evaluar individualmente `mmap`, `mprotect`, `munmap`, `epoll_create1`,
`epoll_ctl`, `epoll_wait`, `futex` y señales. La lista no obliga a
implementarlas: descartar las innecesarias. Cada declaración incluirá una URL o
referencia versionada a documentación oficial/manual del SO y, cuando aplique,
header ABI del target. No adivinar tamaños, tipos, sentinels o valores de error.

**Decisión:** no usar números de syscall ni ABI de kernel directamente en F1.
Las firmas concretas de FFI quedan como gate de la subfase que las implemente;
no se incluyen aquí porque aún no hay integración seleccionada ni prueba del
target.
