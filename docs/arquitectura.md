# Arquitectura

## Interfaz: controles propios sobre Direct2D

Kata dibuja sus propios controles con el estilo Fluent de Windows 11, sobre Direct2D y DirectComposition, mediante la crate oficial `windows` de Microsoft.

**Por qué:**

- **No hay una opción oficial:** WinUI 3, la interfaz nativa de Microsoft, no tiene soporte real para Rust.
- **Son pocos controles:** botón (de acento, estándar y de icono), slider por niveles, interruptor, barra de herramientas con iconos, campo de atajo, icono de información con su globo y lienzo. Los iconos de las herramientas son figuras propias, dibujadas como geometría de Direct2D, así que se ven nítidos a cualquier escala. Hacerlos es abarcable.
- **La barra de herramientas** flota centrada abajo. Al elegir una herramienta, un resalte se desliza hasta ella, estirándose en el camino, y su icono se rellena; la cápsula de sus niveles se desliza a su nuevo ancho, o se hunde y se desvanece si la herramienta no tiene. Con el ajuste de los nombres, cada uno va debajo de su icono; sin él, aparece en un globo al dejar el cursor quieto, igual que el de «i». Dentro de la cápsula de una herramienta puede ir otra barra, más compacta y sin cápsula propia: la de las Formas, para elegir rectángulo, elipse o flecha.
- **El globo de información** aparece cuando el cursor se queda quieto sobre el icono «i», tras el tiempo que Windows fija para eso (`TrackMouseEvent` con `TME_HOVER`), como los globos nativos. Así no parpadea al cruzar el icono, y esperar no cuesta ni un cuadro.
- **El logo de la bienvenida** (`crates/ui/src/logo.rs`) es geometría de Direct2D con las medidas del icono, no una imagen: así puede dibujarse solo. Cada cuadro arma el tramo ya trazado de cada esquina (recta, curva y recta), y el centro crece con un resorte. En contraste alto se dibuja entero, en los colores del sistema.
- **Rendimiento:** no carga ningún framework, por lo que arranca al instante y gasta muy poca memoria.
- **Control total del diseño:** cada animación, sombra y transición se ajusta al detalle. Las animaciones las ejecuta el compositor de Windows, así que siguen fluidas aunque la app esté ocupada.

**Descartado:** Tauri u otras opciones basadas en WebView (más pesadas y no nativas) y Slint (otra capa entre Kata y Windows).

## Accesibilidad

Al dibujar sus propios controles, Kata hace por su cuenta lo que Windows da gratis a los controles estándar:

- **Movimiento:** con «Efectos de animación» apagado en Windows, cada animación salta a su final. Se lee en cada animación, así que el cambio se nota al momento.
- **Teclado:** Tab y Mayús + Tab recorren los controles, marcados con el anillo de foco de Fluent. Enter o Espacio los pulsan; las flechas, Inicio y Fin mueven sliders y selectores.
- **Contraste alto:** con un tema de contraste, los colores salen del tema y la ventana deja Mica por un fondo sólido.
- **Narrador:** cada ventana describe sus controles a Windows con UI Automation (`crates/ui/src/access.rs`): nombre, tipo, estado y lugar, y lo que explica su globo de información, si lo tiene. Lo que pide Narrador (pulsar, elegir, cambiar un nivel) llega a la ventana como una acción de teclado. Solo trabaja si un lector de pantalla lo pide; si no, no cuesta nada.

## Captura

- La pantalla se congela al iniciar el recorte. Todos los monitores, con su escala propia.
- **Hoy:** GDI copia todo el escritorio de una vez. En el mismo instante se anotan las ventanas y monitores, para resaltarlos al pasar el cursor. En las esquinas de cada monitor (8 píxeles a escala 100 %), gana el monitor sobre cualquier ventana: los bordes de la pantalla frenan el cursor ahí, así que se llega de un golpe de ratón.
- **Ventanas tapadas:** de cada ventana se anota también si otra de encima la toca. Si sí, al pasar el cursor un hilo aparte le pide a Windows la ventana entera con `PrintWindow` (`PW_RENDERFULLCONTENT`, el mismo que usa Chromium), en 13–23 ms, y la pinta sobre lo congelado dentro de sus esquinas redondeadas. Se guarda por si se hace clic; si aún no llegó, el clic la espera hasta 500 ms. Si la ventana se movió, no responde o devuelve todo negro (juegos, video protegido), se queda lo congelado. Las ventanas sin nada encima no cambian: GDI basta.
- **La silueta de la bienvenida** (`crates/app/src/overlay/silhouette.rs`): en las primeras capturas, la ventana tapada más al frente se elige al abrir la selección y se pide entera de antemano, para que salga completa en cuanto llega el cursor. Su contorno, con sus esquinas redondeadas, es una sola figura de Direct2D que empieza en el punto más cercano al cursor, puesto en el borde de un píxel para que la costura no se vea. Se prepara una vez; mientras se traza, cada cuadro solo cambia el patrón de guiones del trazo (dos tramos que crecen desde ese punto, uno en cada sentido), y al cerrarse se dibuja entero. Lleva un degradado en diagonal de los colores de firma (`theme::signature`, los mismos del destello, del borde de la tarjeta y del icono de la bienvenida) y una sombra fina debajo; en contraste alto, el color de acento del sistema y sin sombra. Pasado el trazo no repinta nada.
- **HDR:** con HDR encendido, Windows ya le entrega a GDI la pantalla en colores estándar. Ventanas, páginas y texto salen idénticos a como se ven (medido, píxel a píxel). Solo un video en reproducción puede salir más claro. Capturarlo en HDR con Desktop Duplication no compensa: cuesta unos 150 ms y, con la pantalla quieta, el primer cuadro llega en negro.
- **Memoria:** la selección dibuja con su propia fábrica de Direct2D. Al cerrarse, Windows libera sus búferes del tamaño de la pantalla en unos segundos; con la fábrica compartida, se quedaban retenidos.

## Editor

- **Efectos en el procesador:** el desenfoque, el foco, los pasos y las formas se calculan con Rust puro (`crates/editor`). Lo que se ve en pantalla es exactamente lo que se guarda y se copia, y se puede probar sin Windows.
- **Solo lo que cambia:** tras cada edición se recalcula y se envía a la tarjeta gráfica únicamente la zona afectada.
- **En todos los núcleos:** el Foco se dibuja por franjas de filas, una por núcleo, a la vez. Un Foco sobre toda una captura 4K pasó de 55 a 11 ms por cuadro de su animación. El desenfoque también se reparte entre los núcleos.
- **Desenfoque:** un desenfoque gaussiano, como el de Photoshop, con un alcance (desviación típica) de 10, 16 o 26 píxeles por cada 100 % de escala, según la intensidad. Para que cueste lo mismo en cualquier intensidad, primero promedia la captura por celdas tres veces más pequeñas que ese alcance; luego pasa tres cajas por las celdas, a lo ancho y a lo alto, que juntas siguen una campana de Gauss casi exacta, y funde las celdas de vuelta en píxeles con curvas suaves. Las cajas suman números enteros, así que cada píxel sale igual se desenfoque la captura entera o solo un trozo. Al final cada píxel recibe un grano muy fino, el mismo en rojo, verde y azul, que nunca tiñe, evita los escalones en los degradados y hace que el desenfoque no se pueda comparar con un texto adivinado. Medido sobre texto de prueba: el texto no se puede recuperar y ya no deja manchas.
- **Desenfocar alrededor:** cuando el Foco desenfoca lo de alrededor, se guarda una copia de toda la captura desenfocada al nivel elegido, y el Foco la muestra fuera de su área. Al editar debajo, solo se vuelve a desenfocar hasta donde el cambio alcanza. Crearla en una captura 4K tarda unos 15 ms, una sola vez; mover un desenfoque debajo, 7 ms por cuadro.
- **Pasos:** cada número es un disco naranja con borde blanco y cifras redondeadas propias, dibujadas sin fuentes, con bordes suavizados. Mediano, mide 28 píxeles por cada 100 % de escala de la pantalla capturada, como se veía en ella, y nunca menos del 3,6 % del lado corto de la captura, para que se lea cuando una captura grande se achica en un documento. Pequeño, Grande y Muy grande son 0,75, 1,35 y 1,8 veces eso. Todos los pasos comparten el tamaño, así que cambiarlo los redimensiona alrededor de su centro; el borde y la sombra crecen con el disco. Su número es su orden: al borrar uno, los siguientes se renumeran y se redibujan.
- **Formas:** una línea naranja de 6 píxeles por cada 100 % de escala, con borde blanco y sombra suave; crece con la captura igual que los pasos, y comparte su tinta (`crates/editor/src/ink.rs`). Se dibuja con bordes suavizados, calculando la distancia de cada píxel a la línea, sin librerías (`figure.rs`). La flecha une la línea con una punta triangular rellena, de esquinas redondeadas, cuya punta cae justo donde terminó el arrastre; en una flecha corta, la punta se encoge hasta un mínimo y la línea crece hacia atrás poco a poco, sin dejar muñones. Al arrastrar, solo se vuelve a dibujar lo que la tinta toca: el marco alrededor del hueco de un rectángulo o una elipse, o franjas de filas a lo largo de una flecha, y no la captura entera. Una forma se selecciona sobre su línea, así que dentro de ella se sigue pudiendo seleccionar o dibujar.
- **Zoom:** cada paso de la rueda acerca un 25 %, hasta que cada píxel de la captura ocupa 8 de la pantalla. Solo cambia cómo la tarjeta gráfica dibuja la captura, así que no se recalcula ningún píxel. Va y viene en 167 ms; el píxel bajo el cursor no se mueve en todo el camino, porque la posición y la escala avanzan a la par. La píldora de `Espacio` aparece y se va en 250 ms: sube 8 DIPs, crece del 90 al 100 % y se hace opaca, y los nombres de las herramientas suben para dejarle sitio.
- **Entrega:** la captura se copia y se guarda en cuanto se abre el editor. Cada cambio, al terminar, se copia y se guarda de nuevo. Al cerrar, se entrega lo último.
- **Deshacer:** el historial vive solo en memoria, mientras el editor está abierto.

## Guardado

- **Formato:** PNG, con un compresor propio escrito desde cero (`crates/editor`), sin librerías ni código de nadie más. Pesa menos que con el de Windows, sin cambiar un píxel: 18 % de media en las pruebas, 5–8 % en el uso diario y lo mismo con fotos. Cómo funciona, qué se probó y cuánto ahorra: [compresion.md](compresion.md).
- **Comprobado:** una prueba hace que el decodificador de Windows lea cada PNG de vuelta, píxel a píxel.
- **Sin bloqueos:** se guarda en segundo plano, así la app nunca se traba.
- **Escritura atómica:** primero un archivo temporal, luego se reemplaza. Nunca queda una imagen a medias.
- **Portapapeles:** se copia al instante, antes de guardar.
- **Cambiar el nombre:** el hilo que guarda también renombra, en orden con las escrituras, con `MoveFileExW`, que nunca reemplaza un archivo; si el nombre ya existe, prueba ` (2)`, ` (3)`… Lo que Windows no admite en un nombre no se escribe, y `/ \ : |` se vuelven un guion. El cuadro de texto es propio (`crates/editor/src/name.rs` y `crates/app/src/editor/name.rs`): el cursor parpadea al ritmo de Windows y se queda quieto a los 5 s; se selecciona con el ratón, Mayús o Ctrl + A, y se copia, corta y pega solo texto de una línea. En reposo, el nombre deja 28 DIPs a cada lado: ahí aparecen, al pasar el cursor, un asa de seis puntos y un lápiz (de Segoe Fluent Icons) en 167 ms. Al abrir el editor, tras una pausa, lo cruza un destello, y otra vez al cambiarlo: el mismo texto, redibujado con un pincel de degradado fijo (azul, violeta, rosa y naranja, luminosos en el tema oscuro y más intensos en el claro, sin depender del color de acento) que forma una franja inclinada de 96 DIPs, desvanecida en sus bordes (`Painter::glint`), y avanza pareja en 1,4 s, pausa incluida.
- **Arrastrar el archivo:** pasado el umbral de arrastre de Windows, el nombre pide el archivo al hilo que guarda, que lo da solo cuando no le queda nada por escribir ni renombrar; si no, se reintenta con el siguiente movimiento, sin bloquear. El archivo sale como lo arrastra el Explorador: su objeto de datos de shell (`BHID_DataObject`) y `SHDoDragDrop`. La imagen de arrastre es la propia captura, hecha tarjeta (`draw_card`): de hasta 200 × 140 DIPs, con esquinas de 8 DIPs y 3° de inclinación, dos sombras (una honda, de 36 DIPs de difuminado, y otra corta, de 6), un brillo blanco en diagonal y un borde de 2,5 DIPs con los colores del destello. Direct2D la dibuja en un mapa de bits aparte (`Surface::picture`, de 3 a 9 ms, una vez por arrastre) y Windows la lleva con `IDragSourceHelper2`; el cursor la sostiene cerca de su borde de abajo, para que el «Copiar en…» de Windows quede debajo. Si algo falla, Windows pone su imagen de siempre. Windows solo muestra la tarjeta sobre las ventanas que aceptan que se suelte algo, así que, durante el arrastre, el editor se registra como destino (`RegisterDragDrop`) solo para mostrarla, con `IDropTargetHelper`: no acepta nada y quita la señal de prohibido (`DROPIMAGE_NOIMAGE`); al salir, devuelve a Windows esa señal (`DROPIMAGE_INVALID`) y, al terminar, se retira (`RevokeDragDrop`). El despegue lo dibuja el propio editor: en 350 ms, la captura se vuelve tarjeta y vuela hasta el cursor, mezclando a la vez posición, tamaño, inclinación, esquinas y sombras, mientras la cruza el destello. Hasta que aterriza, el editor no le pasa el arrastre a `IDropTargetHelper`, así que Windows no pone su tarjeta encima de la del editor; al aterrizar, o si el cursor sale antes de la ventana, Windows la toma. Si el arrastre termina sin ir a ningún sitio (`DROPEFFECT_NONE`: Esc, o soltarla sobre el editor), la tarjeta vuelve volando del cursor a la captura. Mientras tanto, la captura del editor se achica un 4 % y se atenúa a la mitad, y vuelve al soltar. El arrastre corre después de atender el mensaje, con el estado del editor libre, así que el editor sigue dibujando durante el bucle de `SHDoDragDrop`. Solo se permite copiar. Por eso Kata inicia OLE (`OleInitialize`) y no solo COM.
- **Mapas de bits:** se dibujan píxel por píxel cuando caen derechos y a su tamaño exacto, y con interpolación cúbica si no (con zoom o inclinados, como la tarjeta del arrastre), según la transformación con que se dibujan.

## Ajustes

- **Dónde viven:** en el registro del usuario, `HKCU\Software\Kata`, junto con lo que recuerda el editor. Sin archivos de configuración. Instalada, Windows le da a cada paquete su propia copia, así que Kata y Kata Dev no se mezclan.
- **Atajo:** `RegisterHotKey`. Mientras se elige uno nuevo, el actual se suelta, para que pulsarlo no inicie una captura.
- **Impr Pant y Win + Mayús + S:** Windows se reserva `Alt` y `Win + Impr Pant`, y `Win + Mayús + S` es suyo, así que `RegisterHotKey` no basta. Con el ajuste encendido, un gancho de teclado de bajo nivel (`crates/app/src/screen_keys.rs`), en un hilo propio, se queda con esas teclas antes que Windows; cualquier otra tecla pasa al instante. Si `Win` o `Alt` siguen pulsadas, toca una tecla sin uso para que al soltarlas no se abra Inicio ni un menú. Se descartó declarar el protocolo `ms-screenclip:`: obligaba a elegir Kata en Configuración, no cubría `Alt` ni `Win + Impr Pant` y abría un segundo `Kata.exe` en cada captura.
- **Iniciar con Windows:** la `StartupTask` del paquete. Windows abre Kata sin argumentos; Kata pregunta cómo la abrieron y, si fue al iniciar sesión, espera en la bandeja sin ventanas. Si se apaga en los ajustes de Windows, solo se vuelve a encender ahí. Sin instalar, Kata no inicia con Windows.
- **Una sola instancia:** abrir Kata otra vez le pide a la que ya corre que muestre su ventana.
- **Bienvenida:** el valor `Welcomed` se escribe al terminar la primera captura. Mientras no existe, la ventana principal abre con la bienvenida. El valor `Hints` cuenta las capturas que mostraron la silueta; al seguirla una vez, salta a tres y no vuelve.

## Sonido

- Un toque suave de unos 120 ms, generado por código al arrancar. No hay archivos de audio.
- Ya está en memoria, así que suena sin retraso.

## Distribución

- **Paquete:** MSIX para x64 y ARM64, con `runFullTrust` (bandeja y atajo global) y `StartupTask` (iniciar con Windows).
- **Sin instalaciones extra:** el runtime de C va incluido dentro del ejecutable.
- **Kata y Kata Dev:** dos copias instaladas, separadas por completo. Kata (`AlexPalacios.Kata`) es la que Alex usa a diario, con sus ajustes reales: la instala la Microsoft Store, y él la actualiza desde ahí. Kata Dev (`AlexPalacios.Kata.Dev`) recibe cada cambio para probarlo y solo usa datos de prueba: su propio atajo (`Ctrl + Mayús + D`) y su propia carpeta (`Imágenes\Kata Dev`). Kata sabe cuál es por el nombre de su paquete (`crates/app/src/channel.rs`). Kata Dev se firma con un certificado de desarrollo confiado solo en su PC; Kata, la Store.
- **Fabricarlas:** `packaging/build-msix.ps1` fabrica e instala Kata Dev, subiendo el cuarto número de la versión para instalarse encima. `packaging/update-dev.ps1` además baja los cambios de GitHub y abre Kata Dev. Con `-Store`, el paquete para la Store: la identidad que reservó Partner Center, el número de `Cargo.toml`, x64 y ARM64 en un solo `.msixbundle`, sin firmar (lo firma la Store) y sin instalar.
- **Abiertas a la vez:** cada copia nombra su candado de instancia única y su ventana residente con su paquete, así que una no le cede el paso a la otra. El atajo sí es de todo el sistema: por eso cada una tiene el suyo.
- **Iconos:** las cuatro esquinas del símbolo de Kata (`crates/ui/src/icon.rs`) alrededor de un área iluminada, el Foco. Sin placa, como los iconos de Windows 11, en el naranja caqui de Kata (`#EC5A22`, el de los pasos) con la luz desde arriba a la izquierda. Se dibuja al empaquetar, con los bordes en píxeles enteros, así que se ve nítido incluso a 16 px. En la bandeja van solo las esquinas, en blanco o negro según la barra de tareas, como los iconos de Windows ahí. En Kata Dev, una etiqueta naranja que dice «Dev» cruza la parte de abajo, también en la bandeja, y sus ventanas se llaman «Kata Dev».

## Metas de rendimiento

| Medida | Meta | Medido |
|---|---|---|
| Memoria en reposo | < 15 MB | 10 MB (12 MB tras capturar) |
| CPU en reposo | 0 % | 0 % |
| Atajo → selección visible | < 100 ms | 30–40 ms |
| Aplicar un desenfoque de 1200 × 700 (captura 4K) | < 16 ms | 3,5 ms |
| Cuadro de la animación del Foco (captura 4K) | < 16 ms | 11 ms (peor caso: sobre toda la captura, 12 núcleos) |
| Activar «Desenfocar alrededor» (captura 4K) | < 50 ms | 15 ms (una vez; luego solo lo que cambia) |
| Mover un desenfoque bajo «Desenfocar alrededor» (captura 4K) | < 16 ms | 7 ms por cuadro |
| Cuadro de la bienvenida | < 16 ms | 0,17 ms (solo repinta durante sus 2,35 s) |
| La silueta, por cuadro de la selección | < 1 ms | 0,1–0,2 ms (solo durante su segundo de trazo) |
| Tamaño del ejecutable | < 5 MB | 0.3 MB |
| PNG frente al de Windows | Nunca mayor | 18 % menor de media en las pruebas; con fotos, hasta 0,5 % mayor |
