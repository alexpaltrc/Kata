# Producto

## Capturar

- **Cómo empezar:** con el atajo (`Ctrl + Mayús + S` por defecto, configurable), con el icono de la bandeja o con el botón **Nuevo recorte** de la ventana principal. Con el ajuste encendido, también con `Impr Pant` (sola o con `Alt`, `Win`, `Ctrl` o `Mayús`) y `Win + Mayús + S`, en vez de Recortes.
- **Selección:** la pantalla se congela y se oscurece un poco. Al pasar el cursor se ilumina la ventana que está debajo (o el monitor, sobre el escritorio); un clic la captura. En cualquier esquina se ilumina el monitor entero, con la barra de tareas: es la pantalla completa. Arrastrar elige cualquier área. Una etiqueta muestra el tamaño en píxeles. `Esc` o clic derecho cancelan.
- **Ventanas tapadas:** si otra ventana tapa una parte, al pasar el cursor la ventana aparece completa, y así se captura. Lo que sale de la pantalla no se incluye.
- **Al soltar:** la captura se copia al portapapeles al instante, suena un toque sutil y elegante (se puede desactivar) y se guarda en segundo plano.
- **Después:** según el ajuste, se abre el editor o no aparece nada, para tomar varias capturas seguidas.
- **Ubicación:** `Imágenes\Kata\Kata 2026-10-27 14.32.05.png`, o la carpeta elegida en Ajustes.

## Editar

Solo hay dos herramientas. Se arrastra sobre la captura para usarlas.

1. **Desenfocar.** Uno o varios rectángulos. Intensidad: Suave, Medio, Fuerte. El texto desenfocado no se puede recuperar.
2. **Foco.** Un área queda resaltada y el resto se oscurece. Hay un solo foco: uno nuevo reemplaza al anterior. Ampliación: 100 %, 125 %, 150 %, 200 %.

Reglas:

- Se ve un solo slider, el de la herramienta activa. Tiene pocos niveles y recuerda el último valor usado.
- Un clic selecciona un rectángulo: se mueve arrastrándolo, se redimensiona desde sus esquinas y se borra con el botón de su esquina, `Supr` o `Retroceso`. Mover el slider cambia el rectángulo seleccionado.
- `1` y `2` cambian de herramienta. `Ctrl + Z` deshace; `Ctrl + Y` o `Ctrl + Mayús + Z` rehacen. `Ctrl + S` guarda, `Ctrl + C` copia y `Esc` quita la selección.
- La herramienta, los niveles y el Autoguardado se recuerdan para la siguiente captura.

## Guardado y portapapeles, sin pensar

- **Autoguardado** (interruptor en el editor, activado por defecto): cada cambio sobrescribe el PNG. Al apagarlo aparece el botón **Guardar**.
- **El portapapeles siempre tiene la versión más reciente:** la captura recién tomada y, después, cada edición, aunque el Autoguardado esté apagado.
- **La versión editada reemplaza a la original.** No quedan copias ni archivos extra.
- **Deshacer** funciona mientras el editor está abierto. Al cerrarlo, los cambios quedan fijos en el archivo; con el Autoguardado apagado, lo que no se guardó queda solo en el portapapeles.

## Bandeja

Un clic en el icono inicia un recorte. Su menú: **Nuevo recorte** · **Abrir capturas** · **Ajustes** · **Salir**.

## Ventana principal

El botón **Nuevo recorte**, el atajo debajo y un acceso a Ajustes. Nada más.

- Se abre al iniciar Kata. Si Kata ya está en la bandeja, abrirlo otra vez la muestra.
- **Nuevo recorte** cierra la ventana antes de capturar, para que no salga en la captura.

## Ajustes

Cada cambio se aplica y se guarda al instante. `Esc` o la flecha vuelven al inicio.

| Ajuste | Por defecto | Cómo funciona |
|---|---|---|
| Atajo | `Ctrl + Mayús + S` | Un clic y se pulsa el nuevo. Lleva `Ctrl` o `Alt`, salvo `Impr Pant`, que puede ir solo. Si otra app lo usa, lo avisa. `Esc` cancela. |
| Capturar con Impr Pant y Win + Mayús + S | Desactivado | Esas teclas abren el recorte de Kata en vez de Recortes. Apagado, vuelven a Windows al instante. |
| Abrir el editor al capturar | Activado | Apagado, la captura solo se copia y se guarda. |
| Sonido | Activado | Al encenderlo suena, para oírlo. |
| Carpeta de guardado | `Imágenes\Kata` | **Cambiar** abre el selector de carpetas de Windows. |
| Iniciar con Windows | Desactivado | Kata arranca en la bandeja, sin abrir ventanas. |

## Pendiente

En este orden:

1. **Fallo del Foco.** Kata deja de responder al usar el Foco sobre toda la captura.
2. **Zoom en el editor.** La rueda del ratón acerca y aleja la captura, hacia donde está el cursor. `Ctrl + 0` la devuelve a su tamaño, como en Windows 11. Sin botones.
3. **Identidad definitiva.** El icono sigue siendo provisional: el símbolo de Kata sobre una placa azul.
4. **Compresión a fondo.** El plan por fases de [compresion.md](compresion.md).

## Fuera de alcance por ahora

- **Historial de capturas.** La app no guarda ningún registro de capturas. Se puede evaluar en el futuro.
