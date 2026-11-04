# Producto

## Capturar

- **Cómo empezar:** con el atajo (`Ctrl + Mayús + S` por defecto, configurable), con el icono de la bandeja o con el botón **Nuevo recorte** de la ventana principal. Con el ajuste encendido, también con `Impr Pant` (sola o con `Alt`, `Win`, `Ctrl` o `Mayús`) y `Win + Mayús + S`, en vez de Recortes.
- **Selección:** la pantalla se congela y se oscurece un poco. Al pasar el cursor se ilumina la ventana que está debajo (o el monitor, sobre el escritorio); un clic la captura. En cualquier esquina se ilumina el monitor entero, con la barra de tareas: es la pantalla completa. Arrastrar elige cualquier área. Una etiqueta muestra el tamaño en píxeles. `Esc` o clic derecho cancelan.
- **Ventanas tapadas:** si otra ventana tapa una parte, al pasar el cursor la ventana aparece completa, y así se captura. Lo que sale de la pantalla no se incluye.
- **Al soltar:** la captura se copia al portapapeles al instante, suena un toque sutil y elegante (se puede desactivar) y se guarda en segundo plano.
- **Después:** según el ajuste, se abre el editor o no aparece nada, para tomar varias capturas seguidas.
- **Ubicación:** `Imágenes\Kata\2026-11-03 14.32.05.png`, o la carpeta elegida en Ajustes.

## Editar

Cuatro herramientas, como iconos en una barra centrada abajo. Encima, en otra cápsula, los niveles de la herramienta activa. Al dejar el cursor quieto sobre un icono aparece su nombre, y un ajuste lo muestra siempre debajo.

1. **Desenfocar.** Se arrastra para cubrir uno o varios rectángulos. Intensidad: Suave, Medio, Fuerte. El texto desenfocado no se puede recuperar.
2. **Foco.** Se arrastra sobre un área: queda resaltada y el resto se oscurece. Hay un solo foco: uno nuevo reemplaza al anterior. Ampliación: 100 %, 125 %, 150 %, 200 %. **Desenfocar alrededor:** Ninguno, Suave, Medio o Fuerte, los mismos niveles de Desenfocar, aplicados a todo lo que queda fuera del foco.
3. **Pasos.** Cada clic pone el número siguiente, en un círculo naranja con borde blanco. Borrar un paso renumera los que siguen. Su tamaño se ajusta solo a la captura, para que se lea aunque la captura se achique en un documento. **Tamaño:** Pequeño, Mediano, Grande o Muy grande, el mismo para todos los pasos: cambiarlo cambia los que ya están.
4. **Formas.** Se arrastra para dibujar un rectángulo, una elipse o una flecha, en el naranja de los pasos y con el grosor ajustado a la captura. El rectángulo y la elipse son solo el contorno; la flecha lleva una punta rellena, de esquinas suavizadas. **Forma:** Rectángulo, Elipse o Flecha, elegida con su icono en la cápsula de encima.

Reglas:

- Cada nivel es un slider con pocos pasos, que recuerda el último valor usado.
- Un clic selecciona un rectángulo, un paso o una forma (una forma, sobre su línea, para no tapar lo que rodea): se mueve arrastrándolo, un rectángulo o una forma se redimensiona desde sus esquinas, una flecha desde sus dos puntas, y se borra con el botón de su esquina, `Supr` o `Retroceso`. Mover un slider o elegir otra forma cambia lo seleccionado.
- Un rectángulo que cubre toda la captura no tiene adónde moverse: arrastrar sobre él dibuja uno nuevo con la herramienta activa.
- **Zoom:** la rueda del ratón acerca y aleja la captura, hacia donde está el cursor, nunca menos que lo que cabe en la ventana. `Espacio` o `Ctrl + 0` la devuelven a su tamaño. Sin botones: mientras hay zoom, una píldora encima de la barra dice que `Espacio` lo restablece; surge de la barra al acercar y vuelve a ella al quitar el zoom. Al cambiar el tamaño de la ventana, la captura vuelve a caber.
- `1`, `2`, `3` y `4` cambian de herramienta. `Ctrl + Z` deshace; `Ctrl + Y` o `Ctrl + Mayús + Z` rehacen. `Ctrl + S` guarda, `Ctrl + C` copia y `Esc` quita la selección.
- La herramienta, los niveles y el Autoguardado se recuerdan para la siguiente captura.

## Guardado y portapapeles, sin pensar

- **Autoguardado** (interruptor en el editor, activado por defecto): cada cambio sobrescribe el PNG. Al apagarlo aparece el botón **Guardar**.
- **El portapapeles siempre tiene la versión más reciente:** la captura recién tomada y, después, cada edición, aunque el Autoguardado esté apagado.
- **La versión editada reemplaza a la original.** No quedan copias ni archivos extra.
- **El nombre** va arriba, al centro del editor. Con un clic se escribe; la fecha y la hora se quedan delante, para que todas las capturas, con nombre o sin él, se ordenen solas por antigüedad: `2026-11-03 14.32.05 - Pedidos.png`. El archivo cambia de nombre al terminar (Intro, un clic fuera o al cerrar el editor), con o sin Autoguardado. Esc deja el de antes, y un nombre vacío vuelve a la fecha y la hora. Sin nombre, tras la fecha dice «Ponle nombre», en gris. Al pasar el cursor aparecen un asa de puntos a la izquierda (se arrastra) y un lápiz a la derecha (se renombra). Al abrir el editor lo cruza un destello de colores (azul, violeta, rosa y naranja, siempre los mismos, luminosos en el tema oscuro y más intensos en el claro), para que se note que está ahí, y otra vez al cambiarlo.
- **Arrastrar la captura:** el nombre, arrastrado, lleva el archivo a una carpeta, un documento, un chat o un correo. Se copia: la captura conserva el suyo. La captura misma viaja con el cursor desde el primer movimiento, también sobre el editor, como una tarjeta pequeña de esquinas redondeadas, apenas inclinada; mientras tanto, en el editor se hunde y se atenúa un poco, y vuelve al soltar. Soltarla sobre el editor no hace nada.
- **El botón de la carpeta** (junto al Autoguardado) abre la carpeta de las capturas, con esta seleccionada.
- **Deshacer** funciona mientras el editor está abierto. Al cerrarlo, los cambios quedan fijos en el archivo; con el Autoguardado apagado, lo que no se guardó queda solo en el portapapeles.

## Bandeja

Un clic en el icono inicia un recorte. Su menú: **Nuevo recorte** · **Abrir capturas** · **Ajustes** · **Salir**.

## Ventana principal

El botón **Nuevo recorte**, el atajo debajo y el engranaje de Ajustes, pegado a los botones de la ventana como uno más: mismo ancho, misma altura y el mismo sombreado al pasar el cursor. Nada más.

La barra de título sigue las medidas de Windows 11: 32 píxeles de alto, el título a 16 del borde y, en Ajustes, la flecha de volver a 16 del borde y del título.

- Se abre al iniciar Kata. Si Kata ya está en la bandeja, abrirlo otra vez la muestra.
- **Nuevo recorte** cierra la ventana antes de capturar, para que no salga en la captura.

## Ajustes

Cada cambio se aplica y se guarda al instante. `Esc` o la flecha vuelven al inicio.

| Ajuste | Por defecto | Cómo funciona |
|---|---|---|
| Atajo | `Ctrl + Mayús + S` | Un clic y se pulsa el nuevo. Lleva `Ctrl` o `Alt`, salvo `Impr Pant`, que puede ir solo. Si otra app lo usa, lo avisa. `Esc` cancela. |
| Kata es tu app de capturas | Desactivado | `Impr Pant` (sola o con `Alt`, `Win`, `Ctrl` o `Mayús`) y `Win + Mayús + S` abren el recorte de Kata en vez de Recortes. Apagado, vuelven a Windows al instante. Lo explica el icono «i» de al lado, al dejar el cursor quieto encima. |
| Abrir el editor al capturar | Activado | Apagado, la captura solo se copia y se guarda. |
| Mostrar los nombres de las herramientas | Desactivado | El nombre de cada herramienta, siempre a la vista debajo de su icono. |
| Sonido | Activado | Al encenderlo suena, para oírlo. |
| Carpeta de guardado | `Imágenes\Kata` | **Cambiar** abre el selector de carpetas de Windows. |
| Iniciar con Windows | Desactivado | Kata arranca en la bandeja, sin abrir ventanas. |

## Icono

Cuatro esquinas que enmarcan un área iluminada: una captura y su Foco. En el naranja de Kata, un caqui cálido que contrasta con todo y es el mismo de los pasos; sin placa, como los iconos de Windows 11. En la bandeja, solo las esquinas, en blanco o negro.

## Pendiente

- **Compresión a fondo.** El plan por fases de [compresion.md](compresion.md).
- **Bienvenida (onboarding)**, a la altura del resto de Kata. Enseñará lo que hoy no se ve a simple vista, como arrastrar la captura desde su nombre, y permitirá quitar el interruptor de Autoguardado: Kata guardará siempre.
- **Más idiomas:** al menos cinco antes de vender, y atajos que funcionen con cualquier distribución de teclado.
- **Llegar a Ajustes sin la bandeja.** Hay quien nunca la usa. Alex y Claude evalúan juntos la forma más natural.

## Antes de vender

Kata no se vende hasta tener la bienvenida y al menos cinco idiomas: español, inglés y tres más. Lo decidido:

- **Un solo Premium, en la Microsoft Store**, que desbloquea todo de una vez: unos 10 USD en un pago único, o unos 5 USD al año. El pago único es la opción que conviene.
- **Lo gratis queda completo para el día a día**, sin sentirse obligado a pagar, incluido Kata como app de capturas de Windows (`Impr Pant`). Lo premium es lo que personaliza o va más allá:
  - **Atajo a medida.** Gratis, 3 o 4 a elegir.
  - **Colores a medida** en formas y pasos. Gratis, de 3 a 5.
  - **Dibujo a mano alzada.**
- **Precio justo en cada país.** El precio de referencia se ajusta al poder adquisitivo de cada país con una sola cifra pública para todos: el nivel de precios del Banco Mundial (indicador `PA.NUS.PPPC.RF`, cuánto cuestan las cosas allí comparado con Estados Unidos). Partner Center permite fijar un precio distinto en cada mercado; se redondea a un precio que se lea bien en la moneda local.

## Fuera de alcance por ahora

- **Historial de capturas.** La app no guarda ningún registro de capturas. Se puede evaluar en el futuro.
