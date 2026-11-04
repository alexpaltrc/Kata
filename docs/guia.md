# Guía de Kata

Kata toma capturas de pantalla para manuales, guías e instructivos, y las edita con tres
herramientas: **Desenfocar**, **Foco** y **Pasos**. Es para Windows 11.

Esta guía explica toda la app a quien la usa, y sirve de base para una futura página web. Cómo
está construida va en [arquitectura.md](arquitectura.md). La compresión de sus PNG tiene su
propio documento: [compresion.md](compresion.md).

## Por qué Kata es así

- **Solo lo necesario.** Tres herramientas y pocos ajustes. Nada que haya que aprender.
- **Sin pasos de más.** Cada captura se copia y se guarda sola, y cada edición también.
- **De Windows 11.** Mica, Fluent y Segoe UI Variable, con el tema claro u oscuro y el color de
  acento del sistema.
- **Rápida y ligera.** La selección aparece en 30–40 ms, el programa pesa 0,3 MB y en reposo usa
  10 MB de memoria y nada de procesador.
- **Privada.** No se conecta a internet ni recopila nada.
- **Hecha desde cero.** Incluso su compresión de PNG, que deja las capturas de ventanas más
  ligeras que con Windows, sin perder calidad.

## Capturar

- **Empezar:** con `Ctrl + Mayús + S`, con un clic en el icono de la bandeja o con el botón
  **Nuevo recorte** de la ventana principal. El atajo se puede cambiar.
- **Elegir qué capturar:** la pantalla se congela y se oscurece un poco. Al pasar el cursor se
  ilumina la ventana que hay debajo, o el monitor si está sobre el escritorio; un clic la captura.
  Arrastrar elige cualquier área, y una etiqueta muestra su tamaño en píxeles.
- **Pantalla completa:** lleva el cursor a cualquier esquina del monitor. Se ilumina el monitor
  entero, con la barra de tareas, y un clic lo captura.
- **Cancelar:** `Esc` o clic derecho.
- **Ventanas tapadas:** si otra ventana tapa una parte, al pasar el cursor aparece completa, y así
  se captura. Lo que queda fuera de la pantalla no se incluye.
- **Al soltar:** la captura se copia al portapapeles al instante, suena un toque sutil y se guarda
  en segundo plano. Luego se abre el editor (o nada, si así se eligió en Ajustes, para tomar
  varias capturas seguidas).
- **Varios monitores y HDR:** captura todos los monitores, cada uno con su escala. Con HDR, la
  captura se ve igual que en pantalla; solo un video en reproducción puede salir más claro.

## Editar

Hay tres herramientas, como iconos en una barra centrada abajo. Encima aparecen los niveles de la
que está activa. Al dejar el cursor quieto sobre un icono se ve su nombre; en Ajustes se puede
mostrar siempre.

1. **Desenfocar:** se arrastra para tapar uno o varios rectángulos. Intensidad: Suave, Medio o
   Fuerte. El texto desenfocado no se puede recuperar.
2. **Foco:** se arrastra sobre un área para resaltarla y oscurecer el resto. Hay un solo foco: uno
   nuevo reemplaza al anterior. Ampliación: 100 %, 125 %, 150 % o 200 %. Con la gota de al lado,
   **desenfoca también lo de alrededor**, con los mismos niveles de Desenfocar.
3. **Pasos:** cada clic pone el número siguiente, en un círculo naranja. Si se borra uno, los
   demás se renumeran. Su tamaño se ajusta solo a la captura; al lado está el **tamaño**
   (Pequeño, Mediano o Grande), que cambia todos los pasos a la vez.

- Cada nivel es un control deslizante que recuerda el último valor.
- Un clic selecciona un rectángulo o un paso: se mueve arrastrándolo, un rectángulo cambia de
  tamaño desde sus esquinas, y se borra con el botón de su esquina, `Supr` o `Retroceso`. Mover un
  control deslizante cambia lo seleccionado.
- Si un rectángulo cubre toda la captura, arrastrar sobre él dibuja uno nuevo; un clic lo
  selecciona.
- **Zoom:** la rueda del ratón acerca y aleja la captura hacia donde está el cursor. `Ctrl + 0`
  la devuelve a su tamaño.
- Kata recuerda la herramienta, sus niveles y el Autoguardado para la siguiente captura.

## Guardar y copiar, sin pensar

- **Autoguardado** (activado por defecto): cada cambio se guarda sobre el mismo archivo. Al
  apagarlo aparece el botón **Guardar**.
- **El portapapeles siempre tiene la última versión:** la captura recién tomada y, después, cada
  edición, aunque el Autoguardado esté apagado.
- **Un solo archivo por captura.** La versión editada reemplaza a la original, sin copias extra.
- **Deshacer** funciona mientras el editor está abierto. Al cerrarlo, los cambios quedan en el
  archivo; con el Autoguardado apagado, lo que no se guardó queda solo en el portapapeles.
- **Dónde:** `Imágenes\Kata`, con nombres como `2026-11-03 14.32.05.png`, o en la carpeta
  elegida en Ajustes. El botón de la carpeta, en el editor, la abre con la captura ya seleccionada.
- **Nunca a medias:** cada archivo se escribe aparte y luego reemplaza al anterior, así que nunca
  queda una imagen a medio guardar.

## Atajos de teclado

| Atajo | Qué hace |
|---|---|
| `Ctrl + Mayús + S` | Nueva captura (se puede cambiar) |
| `Impr Pant` (sola o con `Alt`, `Win`, `Ctrl` o `Mayús`), `Win + Mayús + S` | Nueva captura, con el ajuste encendido |
| `Esc` o clic derecho | Cancelar la captura |
| `1` / `2` / `3` | Desenfocar / Foco / Pasos |
| `Ctrl + Z` | Deshacer |
| `Ctrl + Y` o `Ctrl + Mayús + Z` | Rehacer |
| `Ctrl + S` | Guardar |
| `Ctrl + C` | Copiar |
| `Supr` o `Retroceso` | Borrar lo seleccionado |
| `Esc` (en el editor) | Quitar la selección |
| Rueda del ratón | Acercar o alejar la captura |
| `Ctrl + 0` | Devolver la captura a su tamaño |

## Bandeja y ventana principal

- **Bandeja:** un clic en el icono inicia una captura. Su menú: **Nuevo recorte**, **Abrir
  capturas**, **Ajustes** y **Salir**.
- **Ventana principal:** el botón **Nuevo recorte**, el atajo debajo y un acceso a Ajustes. Al
  pulsar Nuevo recorte, la ventana se cierra para no salir en la captura.

## Ajustes

Cada cambio se aplica y se guarda al instante. `Esc` o la flecha vuelven al inicio.

| Ajuste | Por defecto | Qué hace |
|---|---|---|
| Atajo | `Ctrl + Mayús + S` | Un clic y se pulsa el nuevo. Lleva `Ctrl` o `Alt`, salvo `Impr Pant`, que puede ir solo. Si otra app lo usa, lo avisa. |
| Kata es tu app de capturas | Apagado | `Impr Pant` y `Win + Mayús + S` abren la captura de Kata en vez de Recortes. El icono «i» de al lado lo explica. |
| Abrir el editor al capturar | Encendido | Apagado, la captura solo se copia y se guarda. |
| Mostrar los nombres de las herramientas | Apagado | El nombre de cada herramienta, siempre debajo de su icono. |
| Sonido | Encendido | El toque al capturar. |
| Carpeta de guardado | `Imágenes\Kata` | **Cambiar** abre el selector de carpetas de Windows. |
| Iniciar con Windows | Apagado | Kata arranca en la bandeja, sin abrir ventanas. |

## Accesibilidad

- **Teclado:** `Tab` y `Mayús + Tab` recorren los controles; `Enter` o `Espacio` los pulsan; las
  flechas, `Inicio` y `Fin` mueven controles deslizantes y selectores.
- **Narrador:** describe cada control con su nombre, tipo y estado.
- **Contraste alto:** Kata usa los colores del tema de contraste.
- **Menos movimiento:** con «Efectos de animación» apagado en Windows, las animaciones saltan a su
  final.

## Privacidad

Kata no recopila, guarda ni envía datos personales, y no se conecta a internet. Las capturas y los
ajustes se quedan en tu equipo. Política completa: [privacidad.md](privacidad.md).

## Preguntas frecuentes

**¿Dónde están mis capturas?** En `Imágenes\Kata`, o en la carpeta elegida en Ajustes. El menú de
la bandeja tiene **Abrir capturas**.

**¿Cómo capturo la pantalla completa?** Durante la captura, lleva el cursor a una esquina del
monitor y haz clic. Entra todo, también la barra de tareas.

**¿La captura pierde calidad?** No. Se guarda en PNG, sin pérdida: cada píxel queda idéntico.

**¿Por qué mis archivos pesan menos?** Kata tiene su propia compresión PNG, hecha desde cero. En
capturas de ventanas y páginas suele ahorrar entre 5 y 20 % frente al codificador de Windows; con
fotos, pesa prácticamente lo mismo. Detalles en [compresion.md](compresion.md).

**¿Puedo usar la tecla Impr Pant?** Sí. Enciende «Kata es tu app de capturas» en Ajustes. Desde
entonces, `Impr Pant` (sola o con `Alt`, `Win`, `Ctrl` o `Mayús`) y `Win + Mayús + S` abren Kata.
Al apagarlo, esas teclas vuelven a Windows al instante.

**¿Guarda un historial de capturas?** No. Kata no lleva ningún registro de lo que capturas.

**¿Por qué solo tres herramientas?** Porque para un manual casi siempre basta con ocultar algo,
señalarlo y numerar los pasos. Menos opciones, menos que aprender.
