# Producto

## Capturar

- **Cómo empezar:** con el atajo (`Ctrl + Mayús + S` por defecto, configurable), con el icono de la bandeja o con el botón **Nuevo recorte** de la ventana principal. Con el ajuste encendido, también con `Impr Pant` (sola o con `Alt`, `Win`, `Ctrl` o `Mayús`) y `Win + Mayús + S`, en vez de Recortes.
- **Selección:** la pantalla se congela y se oscurece un poco. Al pasar el cursor se ilumina la ventana que está debajo (o el monitor, sobre el escritorio); un clic la captura. En cualquier esquina se ilumina el monitor entero, con la barra de tareas: es la pantalla completa. Arrastrar elige cualquier área. Una etiqueta muestra el tamaño en píxeles. `Esc` o clic derecho cancelan.
- **Ventanas tapadas:** si otra ventana tapa una parte, al pasar el cursor la ventana aparece completa, y así se captura. Lo que sale de la pantalla no se incluye.
- **Al soltar:** la captura se copia al portapapeles al instante, suena un toque sutil y elegante (se puede desactivar) y se guarda en segundo plano.
- **Después:** según el ajuste, se abre el editor o no aparece nada, para tomar varias capturas seguidas.
- **Ubicación:** `Imágenes\Kata\2026-11-03 14.32.05.png`, o la carpeta elegida en Ajustes.

## Editar

Cuatro herramientas, como iconos en una barra centrada abajo: primero las que añaden algo encima (Formas y Pasos) y luego las que cambian la imagen (Foco y Desenfocar). Sus opciones no están a la vista: salen de cada herramienta al pasar el cursor por ella (o al elegirla con el teclado) y vuelven a entrar al irse. Lo que la herramienta añade (rectángulo, elipse o flecha) va en un arco alrededor de ella, como la parte de arriba de una rueda; el tamaño de los pasos, en una cápsula encima. Foco y Desenfocar no tienen opciones. Cambiar una opción elige esa herramienta. Al dejar el cursor quieto sobre un icono aparece su nombre, y un ajuste lo muestra siempre debajo.

1. **Formas.** Se arrastra para dibujar un rectángulo, una elipse o una flecha, en el naranja de los pasos y con el grosor ajustado a la captura. El rectángulo y la elipse son solo el contorno; la flecha lleva una punta rellena, de esquinas suavizadas. **Forma:** Rectángulo, Elipse o Flecha, elegida con su icono en la cápsula de encima.
2. **Pasos.** Cada clic pone el número siguiente, en un círculo naranja con borde blanco. Borrar un paso renumera los que siguen. Su tamaño se ajusta solo a la captura, para que se lea aunque la captura se achique en un documento. **Tamaño:** Pequeño, Mediano, Grande o Muy grande, el mismo para todos los pasos: cambiarlo cambia los que ya están.
3. **Foco.** Se arrastra sobre lo único que se quiere ver: esa área queda nítida y todo lo demás se oscurece y se desenfoca, con la intensidad de Desenfocar. Hay un solo foco: uno nuevo reemplaza al anterior. Siempre igual, sin opciones: sin borde, con las esquinas redondeadas y ampliado un 125 %, lo justo para que se despegue de la captura; si no cabe ampliado, como en una captura muy pequeña, crece solo hasta donde cabe. Para señalar algo sin ocultar lo demás están las Formas.
4. **Desenfocar.** Se arrastra para cubrir uno o varios rectángulos. Siempre con la misma intensidad, la más fuerte: nadie se detiene a elegir cuánto desenfocar, solo quiere que no se lea. El texto desenfocado no se puede recuperar.

Reglas:

- Cada nivel es un slider con pocos pasos, que recuerda el último valor usado.
- Un clic selecciona un rectángulo, un paso o una forma (una forma, sobre su línea, para no tapar lo que rodea): se mueve arrastrándolo, un rectángulo o una forma se redimensiona desde sus esquinas, una flecha desde sus dos puntas, y se borra con el botón de su esquina, `Supr` o `Retroceso`. Mover un slider o elegir otra forma cambia lo seleccionado.
- Un rectángulo que cubre toda la captura no tiene adónde moverse: arrastrar sobre él dibuja uno nuevo con la herramienta activa.
- **Zoom:** la rueda del ratón acerca y aleja la captura, hacia donde está el cursor, nunca menos que lo que cabe en la ventana. `Espacio` o `Ctrl + 0` la devuelven a su tamaño. Mientras hay zoom, una píldora encima de la barra dice que `Espacio` lo restablece; surge de la barra al acercar y vuelve a ella al quitar el zoom. Al cambiar el tamaño de la ventana, la captura vuelve a caber.
- **Recortar así:** en la misma píldora, tras una raya, un botón deja solo lo que se ve. Al pasar el cursor, cuatro esquinas con los colores del destello marcan lo que se queda. Al pulsarlo, la vista se aleja hasta mostrar la captura entera, con lo que sobra atenuado; los bordes se cierran sobre lo que queda, que se desliza a su sitio con un resorte, y las esquinas se abren y se apagan. Los pasos y formas que quedan del todo fuera se van con lo recortado, y los pasos que quedan se renumeran. `Ctrl + Z` lo devuelve todo, con el zoom de antes. Se guarda y se copia solo, como cualquier cambio.
- **Juntar capturas:** se arrastra otra captura (una imagen desde el Explorador de archivos) sobre el editor, y viaja como tarjeta, inclinada, bajo el cursor. Junto al lado de una captura, esa mitad se ilumina con las esquinas del destello, como al acomodar ventanas en Windows; junto al borde de la composición, la mitad de toda ella. Al soltarla, la zona se encoge hasta su sitio, la tarjeta se endereza y se posa con un resorte, las demás capturas se hacen a un lado y las esquinas se abren y se apagan. La composición conserva el tamaño de la primera captura: cada captura que llega toma la mitad del espacio donde se suelta, y cada una llena el suyo, ampliada lo justo, con una junta blanca fina entre ellas. Lados distintos arman cualquier composición, y los pasos y formas viajan con su captura. `Ctrl + Z` la quita con el mismo movimiento al revés. Se guarda y se copia sola, como cualquier cambio.
- **Repartir el espacio:** al pasar el cursor por una junta, aparece en ella un asa gris y el cursor de cambiar tamaño. Se arrastra para darle más sitio a un lado, sin bajar del 15 % para ninguno; mientras se sostiene, el asa crece un poco y se vuelve violeta.
- **Reencuadrar:** doble clic sobre una captura de la composición. Las demás se atenúan y lo que su espacio deja fuera se despliega alrededor, tenue, con una línea violeta en su borde y el espacio marcado en violeta. Se arrastra para moverla, la rueda la amplía hasta 5 veces alrededor del cursor y otro doble clic la vuelve a centrar sin ampliar. Los pasos y formas se mueven con ella y nunca quedan fuera de la vista. Enter, Esc o un clic fuera terminan, y lo que sobra se repliega en su espacio. Cada ajuste se deshace por separado con `Ctrl + Z`.
- `1`, `2`, `3` y `4` cambian de herramienta. `Ctrl + Z` deshace; `Ctrl + Y` o `Ctrl + Mayús + Z` rehacen. `Ctrl + S` guarda, `Ctrl + C` copia y `Esc` quita la selección.
- La herramienta y los niveles se recuerdan para la siguiente captura.

## Guardado y portapapeles, sin pensar

- **Se guarda solo, siempre:** cada cambio sobrescribe el PNG al terminar. No hay interruptor ni botón Guardar: nadie tiene que pensar en guardar, y el nombre tiene todo el sitio de la barra.
- **El portapapeles siempre tiene la versión más reciente:** la captura recién tomada y, después, cada edición.
- **La versión editada reemplaza a la original.** No quedan copias ni archivos extra.
- **El nombre** va arriba, al centro del editor. Con un clic se escribe; la fecha y la hora se quedan delante, para que todas las capturas, con nombre o sin él, se ordenen solas por antigüedad: `2026-11-03 14.32.05 - Pedidos.png`. El archivo cambia de nombre al terminar (Intro, un clic fuera o al cerrar el editor). Esc deja el de antes, y un nombre vacío vuelve a la fecha y la hora. Sin nombre, tras la fecha dice «Ponle nombre», en gris. Al pasar el cursor aparecen un asa de puntos a la izquierda (se arrastra) y un lápiz a la derecha (se renombra). Al abrir el editor lo cruza un destello de colores (azul, violeta, rosa y naranja, siempre los mismos, luminosos en el tema oscuro y más intensos en el claro), para que se note que está ahí, y otra vez al cambiarlo.
- **Arrastrar la captura:** el nombre, arrastrado, lleva el archivo a una carpeta, un documento, un chat o un correo. Se copia: la captura conserva el suyo. Al primer movimiento, la captura despega: se vuelve una tarjeta pequeña de esquinas redondeadas, apenas inclinada, y vuela hasta el cursor mientras la cruza el destello del nombre. La tarjeta lleva el sello de Kata: una sombra honda y suave, un brillo leve y un borde fino con los colores del destello. Desde ahí viaja con el cursor, también sobre el editor; mientras tanto, la captura se hunde y se atenúa un poco, y vuelve al soltar. Si se suelta sobre el editor o se cancela con Esc, la tarjeta vuelve volando a la captura.
- **El botón de la carpeta** va abajo, en una isla propia junto a la barra de herramientas: es parte de la barra, pero no una herramienta. Abre la carpeta de las capturas, con esta seleccionada. Al pie de la carpeta, un check dice que la captura está guardada: con cada guardado se vuelve a trazar, en el primero de los colores de firma, y se calienta hasta el naranja de Kata, como las esquinas del icono. Dura 0,7 s y no dice nada más: quien edita mucho solo ve un trazo breve.
- **El botón «+»** va en otra isla, antes de la barra: toma otra captura, con el editor fuera de su camino, igual que el atajo. Arrastrado sobre la captura, ilumina el lugar donde aterrizaría una nueva, como al arrastrar una imagen desde el Explorador; al soltarlo ahí, Kata toma la captura y esta llega a ese lugar como una tarjeta desde el cursor. Si la captura se cancela, el editor vuelve. Juntar capturas es de Premium.
- **Deshacer** funciona mientras el editor está abierto. Al cerrarlo, los cambios quedan fijos en el archivo.

## Bandeja

Un clic en el icono inicia un recorte. Su menú: **Nuevo recorte** · **Abrir capturas** · **Ajustes** · **Salir**.

## Ventana principal

El botón **Nuevo recorte** y, debajo, las teclas que inician una captura, como teclas: el atajo o, con Kata como app de capturas, `Impr Pant`. Alrededor, las cuatro esquinas del icono, en su naranja, como alrededor de una captura; al pasar el cursor por el botón se cierran un poco, con un resorte, como al enfocar. En la barra de título, el icono de Kata y su nombre, y el engranaje de Ajustes antes de los botones de la ventana y distinto de ellos: un botón redondeado, propio. Nada más.

La barra de título mide 48 píxeles de alto, la más alta de las dos que usa Windows 11, igual que la del editor. El título, el engranaje, la flecha de volver y los botones de la ventana comparten la misma línea, en su centro. Los botones de minimizar, maximizar y cerrar los dibuja Kata, del alto de la barra; Windows los sigue reconociendo, así que al dejar el cursor sobre Maximizar salen los diseños de acople.

- Se abre al iniciar Kata. Si Kata ya está en la bandeja, abrirlo otra vez la muestra.
- **Nuevo recorte** cierra la ventana antes de capturar, para que no salga en la captura.

## Ajustes

Cada cambio se aplica y se guarda al instante. `Esc` o la flecha vuelven al inicio. Los ajustes van en tres grupos con título, cada uno en una tarjeta con una línea fina entre sus filas y un icono al principio de cada fila: **Capturar** (atajo, app de capturas, sonido, iniciar con Windows), **Editar** (abrir el editor, nombres de las herramientas) y **Guardar** (carpeta).

| Ajuste | Por defecto | Cómo funciona |
|---|---|---|
| Atajo | `Ctrl + Mayús + S` | Un clic y se pulsa el nuevo. Lleva `Ctrl` o `Alt`, salvo `Impr Pant`, que puede ir solo. Si otra app lo usa, lo avisa. `Esc` cancela. |
| Haz de Kata tu app de capturas | Desactivado | Las teclas de captura de Windows, `Impr Pant` (sola o con `Alt`, `Win`, `Ctrl` o `Mayús`) y `Win + Mayús + S`, abren el recorte de Kata en vez de Recortes. Se llama igual que en Lista. Apagado, vuelven a Windows al instante. Lo explica el icono «i» de al lado, al dejar el cursor quieto encima. |
| Abrir el editor al capturar | Activado | Apagado, la captura solo se copia y se guarda. |
| Mostrar los nombres de las herramientas | Desactivado | El nombre de cada herramienta, siempre a la vista debajo de su icono. |
| Color de formas y pasos | Naranja | Cinco colores listos: naranja, rojo, rosa, azul y verde, igual de vivos y con el blanco del número siempre legible. El sexto es de Premium: un tono a elegir entre todos, con un deslizador, que Kata mantiene vivo y nunca tan claro que el número se pierda. Vale para las capturas que se editen después. |
| Sonido | Activado | Al encenderlo suena, para oírlo. |
| Carpeta de guardado | `Imágenes\Kata` | **Cambiar** abre el selector de carpetas de Windows. |
| Iniciar con Windows | Desactivado | Kata arranca en la bandeja, sin abrir ventanas. |

## Bienvenida

Hasta la primera captura, la ventana principal abre con la bienvenida. El icono se dibuja solo: cada esquina se traza en un color de firma, una tras otra, y se calienta hasta el naranja de Kata pasando por los colores que siguen al suyo; luego se enciende el centro. Llegan «Hola, soy Kata» y «Gratis y sin límites», el botón **Toma tu primera captura** con el atajo debajo, y el destello cruza el saludo. Si se cierra sin capturar, vuelve a aparecer la próxima vez.

En los primeros editores, tres señales, de una en una:

- **El nombre.** «Ponle nombre» se escribe solo, letra a letra y con su cursor, y luego lo cruza el destello.
- **Se guarda solo.** Al guardarse la primera edición, el botón de la carpeta late y de encima le sale un globo, como los de Windows, que dice «Guardado automáticamente». El globo vuelve a entrar, y su isla se abre hacia la derecha en una píldora con el nombre de la carpeta, la cruza el destello y se cierra. Si la píldora no cabe junto al nombre, solo sale el globo.
- **Se arrastra desde el nombre.** En cuanto el nombre termina de escribirse solo y nada más se mueve, el nombre se ve como bajo el cursor, con su asa, y la captura sube como tarjeta y queda colgando bajo él. Entonces el nombre se desplaza un poco, como si alguien lo arrastrara, con la tarjeta siguiéndolo un instante después, y vuelve; la tarjeta regresa a su sitio. Si se empieza a editar antes, lo hace la primera vez que el cursor pasa por el nombre. Si se arrastra en ese momento, la tarjeta sale desde donde está.

Cada una se repite hasta que se usa una vez (renombrar, abrir la carpeta, arrastrar), y como mucho en tres editores. Esperan a que nada más se mueva. Con los efectos de animación apagados no aparecen, y el nombre ya está escrito.

Al cerrar el editor de la primera captura, la ventana principal vuelve con **Lista**. La captura llega como la tarjeta del arrastre: sube, despega inclinada y la cruza el destello. Debajo, «Lista en 0,09 s», lo que tardó esa captura en aparecer desde que se pidió, y «Y pesa un 41 % menos que con Windows», medido guardando esa misma captura también con el codificador de Windows. Si esa cuenta aún no termina, la línea llega en cuanto lo hace, y si la diferencia no llega al medio por ciento, no se dice. Luego, el ajuste **Haz de Kata tu app de capturas**, con «Las teclas de captura de Windows, como Impr Pant, abrirán Kata» debajo, y **Continuar**, con «Todo se queda en tu equipo» debajo. Con el editor apagado, Lista llega en cuanto se guarda la captura.

Después viene **Premium**: «Todo esto es gratis», con el destello, y «Para siempre y sin límites». Luego, en una tarjeta, lo que Premium añade (un atajo a tu medida, tus colores en formas y pasos, varias capturas juntas en una), una línea tras otra, cada una con su icono en un color de firma. Al final, **Seguir gratis** y **Obtener Premium**, lado a lado y del mismo tamaño y peso. Se cierra con la X de la ventana, y `Esc` lleva al inicio. Mientras Kata no se vende, Premium solo aparece en Kata Dev, y la compra aún no está conectada: allí, **Obtener Premium** solo lo da por comprado, para ver cómo se muestra, y «Repetir la bienvenida» lo olvida.

En Ajustes, arriba de todo, una tarjeta lleva a esta página: una corona, «Kata Premium», «Un solo pago, tuyo para siempre» y el botón **Obtener**. Con Premium ya comprado, la tarjeta dice «Eres Premium» y «De por vida. Gracias por apoyar a Kata.», la corona toma el naranja de Kata y la tarjeta lleva un borde con los colores de firma; su botón, **Ver**, abre la página, que entonces da las gracias («Gracias por ser Premium», «Es tuyo de por vida») y, bajo «Ya puedes usar», dice dónde está cada función. El dibujo a mano alzada se descartó.

## Icono

Cuatro esquinas que enmarcan un área iluminada: una captura y su Foco. En el naranja de Kata, un caqui cálido que contrasta con todo y es el mismo de los pasos; sin placa, como los iconos de Windows 11. En la bandeja, solo las esquinas, en blanco o negro.

## Pendiente

- **Compresión: lo que sigue.** Lo más prometedor del plan ya está hecho; las ideas que quedan, en [compresion.md](compresion.md#lo-que-sigue).
- **Bienvenida (onboarding).** Sus cinco fases (hola, la primera captura, el editor, Lista y Premium) están hechas: ver [Bienvenida](#bienvenida). Falta que Alex las pruebe y conectar «Obtener Premium» con la compra en la Store.
- **Más idiomas:** al menos cinco antes de vender, y atajos que funcionen con cualquier distribución de teclado.
- **Juntar capturas: lo que sigue.** Pegar con `Ctrl + V` una captura para juntarla y que la ventana crezca con la composición.
- **Grabar video, a futuro.** Antes de decidir nada, analizar si Kata podría grabar la pantalla: qué permite Windows, si la app seguiría siendo igual de ligera y rápida, y si no la desvía de su enfoque (capturas para manuales). Con eso, Alex decide.
- **Ajustes: cómo se llega y dónde está el engranaje.** Hay quien nunca usa la bandeja, y el engranaje de la ventana principal puede estar mejor puesto. Van de la mano: Alex y Claude evalúan juntos la forma más natural de llegar a Ajustes.

## Antes de vender

Kata no se vende hasta tener la bienvenida y al menos cinco idiomas: español, inglés y tres más. Lo decidido:

- **Un solo Premium, en la Microsoft Store**, que desbloquea todo de una vez: unos 10 USD en un pago único, o unos 5 USD al año. El pago único es la opción que conviene.
- **Lo gratis queda completo para el día a día**, sin sentirse obligado a pagar, incluido Kata como app de capturas de Windows (`Impr Pant`). Lo premium es lo que personaliza o va más allá:
  - **Atajo a medida.** Gratis, el que viene y `Impr Pant` con Kata como app de capturas.
  - **Color a medida** en formas y pasos. Gratis, cinco.
  - **Juntar varias capturas en una.**
- **Probar antes de pagar.** Cada una de las tres se puede usar gratis tres veces, sin registro ni reloj: elegir un atajo propio, elegir un color propio y juntar una captura. Ajustes dice cuántas pruebas quedan bajo el atajo y el color. Al acabarse, usarla lleva a la página de Premium. Mientras Kata no se vende, las pruebas solo cuentan en Kata Dev; en Kata todo es gratis.
- **Precio justo en cada país.** El precio de referencia se ajusta al poder adquisitivo de cada país con una sola cifra pública para todos: el nivel de precios del Banco Mundial (indicador `PA.NUS.PPPC.RF`, cuánto cuestan las cosas allí comparado con Estados Unidos). Partner Center permite fijar un precio distinto en cada mercado; se redondea a un precio que se lea bien en la moneda local.

## Fuera de alcance por ahora

- **Historial de capturas.** La app no guarda ningún registro de capturas. Se puede evaluar en el futuro.
