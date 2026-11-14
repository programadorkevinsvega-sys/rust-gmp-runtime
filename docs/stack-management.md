# Stack Management

## Requisitos

Cada G requiere stack independiente de dirección estable y liberación solo tras
finalizar toda posibilidad de uso. Los stacks no se copian ni reubican. Su owner
RAII permanece asociado a G; un waiter, handle o backend nativo pendiente debe
impedir su liberación.

## Decisión inicial: stack bounded, sin crecimiento

La primera implementación tendrá capacidad fija y acotada por stack, con el
tamaño predeterminado sujeto a medición antes de estabilizar la API. No habrá
crecimiento automático, stack copying, ni handler de señal que intente recuperar
un overflow. Si una G toca la zona de guardia, el comportamiento de seguridad es
fail-stop del proceso por fault no recuperado; nunca continuar con stack
corrupto ni convertir el fault en un panic recuperable.

Linux x86_64 usará dirección estable, reserva de espacio y guardia inaccesible
como estrategia de referencia. La implementación debe verificar alineación,
page size, dirección de crecimiento y el red zone de la ABI. Se compararán
reservas individuales con slabs; `mprotect` por slot puede dividir VMAs, por lo
que no se presumirá reducción de `vm.max_map_count`. El baseline seguro no
promete cientos de miles de stacks.

Los valores de 8 KiB iniciales y 256 KiB reservados son únicamente candidatos
de benchmark, no defaults aprobados. Medir perfiles con `println!`/formateo,
recursión, locals grandes y profundidad. Separar espacio virtual reservado,
páginas tocadas/comprometidas y RSS.

## Pools y liberación

No reutilizar stacks en la primera implementación. Primero demostrar que G es
terminal, no quedan handles de ejecución ni operaciones pendientes, y la región
vuelve a un estado conocido. Pool, limpieza, retención máxima y devolución de
slabs al SO requieren pruebas/modelos y medición antes de habilitarse.

## Ciclo de vida y pruebas

Documentar alineación, límites, propietario, protección, liberación y
reutilización antes del código F2. La matriz F2/F12 debe cubrir stacks pequeños,
overflow en un proceso aislado, guard page, recursión, grandes variables
locales, errores de reserva, direcciones estables, fugas, contaminación al
reuso y límites de VMA. No ejecutar el caso fatal de overflow en el proceso
principal del test runner ni tratar una guardia como prueba de crecimiento.
