# Arquitectura

## Interfaz: controles propios sobre Direct2D

Kata dibuja sus propios controles con el estilo Fluent de Windows 11, sobre Direct2D y DirectComposition, mediante la crate oficial `windows` de Microsoft.

**Por qué:**

- **No hay una opción oficial:** WinUI 3, la interfaz nativa de Microsoft, no tiene soporte real para Rust.
- **Son pocos controles:** botón (de acento, estándar y de icono), slider por niveles, interruptor, selector de herramienta, campo de atajo y lienzo. Hacerlos es abarcable.
- **Rendimiento:** no carga ningún framework, por lo que arranca al instante y gasta muy poca memoria.
- **Control total del diseño:** cada animación, sombra y transición se ajusta al detalle. Las animaciones las ejecuta el compositor de Windows, así que siguen fluidas aunque la app esté ocupada.

**Descartado:** Tauri u otras opciones basadas en WebView (más pesadas y no nativas) y Slint (otra capa entre Kata y Windows).

## Accesibilidad

Al dibujar sus propios controles, Kata hace por su cuenta lo que Windows da gratis a los controles estándar:

- **Movimiento:** con «Efectos de animación» apagado en Windows, cada animación salta a su final. Se lee en cada animación, así que el cambio se nota al momento.
- **Teclado:** Tab y Mayús + Tab recorren los controles, marcados con el anillo de foco de Fluent. Enter o Espacio los pulsan; las flechas, Inicio y Fin mueven sliders y selectores.
- **Contraste alto:** con un tema de contraste, los colores salen del tema y la ventana deja Mica por un fondo sólido.
- **Narrador:** cada ventana describe sus controles a Windows con UI Automation (`crates/ui/src/access.rs`): nombre, tipo, estado y lugar. Lo que pide Narrador (pulsar, elegir, cambiar un nivel) llega a la ventana como una acción de teclado. Solo trabaja si un lector de pantalla lo pide; si no, no cuesta nada.

## Captura

- La pantalla se congela al iniciar el recorte. Todos los monitores, con su escala propia.
- **Hoy:** GDI copia todo el escritorio de una vez. En el mismo instante se anotan las ventanas y monitores, para resaltarlos al pasar el cursor. En las esquinas de cada monitor (8 píxeles a escala 100 %), gana el monitor sobre cualquier ventana: los bordes de la pantalla frenan el cursor ahí, así que se llega de un golpe de ratón.
- **Ventanas tapadas:** de cada ventana se anota también si otra de encima la toca. Si sí, al pasar el cursor un hilo aparte le pide a Windows la ventana entera con `PrintWindow` (`PW_RENDERFULLCONTENT`, el mismo que usa Chromium), en 13–23 ms, y la pinta sobre lo congelado dentro de sus esquinas redondeadas. Se guarda por si se hace clic; si aún no llegó, el clic la espera hasta 500 ms. Si la ventana se movió, no responde o devuelve todo negro (juegos, video protegido), se queda lo congelado. Las ventanas sin nada encima no cambian: GDI basta.
- **HDR:** con HDR encendido, Windows ya le entrega a GDI la pantalla en colores estándar. Ventanas, páginas y texto salen idénticos a como se ven (medido, píxel a píxel). Solo un video en reproducción puede salir más claro. Capturarlo en HDR con Desktop Duplication no compensa: cuesta unos 150 ms y, con la pantalla quieta, el primer cuadro llega en negro.
- **Memoria:** la selección dibuja con su propia fábrica de Direct2D. Al cerrarse, Windows libera sus búferes del tamaño de la pantalla en unos segundos; con la fábrica compartida, se quedaban retenidos.

## Editor

- **Efectos en el procesador:** el desenfoque y el foco se calculan con Rust puro (`crates/editor`). Lo que se ve en pantalla es exactamente lo que se guarda y se copia, y se puede probar sin Windows.
- **Solo lo que cambia:** tras cada edición se recalcula y se envía a la tarjeta gráfica únicamente la zona afectada.
- **Entrega:** la captura se copia y se guarda en cuanto se abre el editor. Cada cambio, al terminar, se copia de nuevo (y, con Autoguardado, se guarda). Al cerrar, se entrega lo último.
- **Deshacer:** el historial vive solo en memoria, mientras el editor está abierto.

## Guardado

- **Formato:** PNG, con un compresor propio escrito desde cero (`crates/editor`), sin librerías ni código de nadie más. Pesa menos que con el de Windows, sin cambiar un píxel: 18 % de media en las pruebas, 5–8 % en el uso diario y lo mismo con fotos. Cómo funciona, qué se probó y cuánto ahorra: [compresion.md](compresion.md).
- **Comprobado:** una prueba hace que el decodificador de Windows lea cada PNG de vuelta, píxel a píxel.
- **Sin bloqueos:** se guarda en segundo plano, así la app nunca se traba.
- **Escritura atómica:** primero un archivo temporal, luego se reemplaza. Nunca queda una imagen a medias.
- **Portapapeles:** se copia al instante, antes de guardar.

## Ajustes

- **Dónde viven:** en el registro del usuario, `HKCU\Software\Kata`, junto con lo que recuerda el editor. Sin archivos de configuración. Instalada, Windows le da a cada paquete su propia copia, así que Kata y Kata Dev no se mezclan.
- **Atajo:** `RegisterHotKey`. Mientras se elige uno nuevo, el actual se suelta, para que pulsarlo no inicie una captura.
- **Impr Pant y Win + Mayús + S:** Windows se reserva `Alt` y `Win + Impr Pant`, y `Win + Mayús + S` es suyo, así que `RegisterHotKey` no basta. Con el ajuste encendido, un gancho de teclado de bajo nivel (`crates/app/src/screen_keys.rs`), en un hilo propio, se queda con esas teclas antes que Windows; cualquier otra tecla pasa al instante. Si `Win` o `Alt` siguen pulsadas, toca una tecla sin uso para que al soltarlas no se abra Inicio ni un menú. Se descartó declarar el protocolo `ms-screenclip:`: obligaba a elegir Kata en Configuración, no cubría `Alt` ni `Win + Impr Pant` y abría un segundo `Kata.exe` en cada captura.
- **Iniciar con Windows:** la `StartupTask` del paquete. Windows abre Kata sin argumentos; Kata pregunta cómo la abrieron y, si fue al iniciar sesión, espera en la bandeja sin ventanas. Si se apaga en los ajustes de Windows, solo se vuelve a encender ahí. Sin instalar, Kata no inicia con Windows.
- **Una sola instancia:** abrir Kata otra vez le pide a la que ya corre que muestre su ventana.

## Sonido

- Un toque suave de unos 120 ms, generado por código al arrancar. No hay archivos de audio.
- Ya está en memoria, así que suena sin retraso.

## Distribución

- **Paquete:** MSIX para x64 y ARM64, con `runFullTrust` (bandeja y atajo global) y `StartupTask` (iniciar con Windows).
- **Sin instalaciones extra:** el runtime de C va incluido dentro del ejecutable.
- **Kata y Kata Dev:** dos copias instaladas, separadas por completo. Kata (`AlexPalacios.Kata`) es la que Alex usa a diario, con sus ajustes reales: la instala la Microsoft Store, y él la actualiza desde ahí. Kata Dev (`AlexPalacios.Kata.Dev`) recibe cada cambio para probarlo y solo usa datos de prueba: su propio atajo (`Ctrl + Mayús + D`) y su propia carpeta (`Imágenes\Kata Dev`). Kata sabe cuál es por el nombre de su paquete (`crates/app/src/channel.rs`). Kata Dev se firma con un certificado de desarrollo confiado solo en su PC; Kata, la Store.
- **Fabricarlas:** `packaging/build-msix.ps1` fabrica e instala Kata Dev, subiendo el cuarto número de la versión para instalarse encima. `packaging/update-dev.ps1` además baja los cambios de GitHub y abre Kata Dev. Con `-Store`, el paquete para la Store: la identidad que reservó Partner Center, el número de `Cargo.toml`, x64 y ARM64 en un solo `.msixbundle`, sin firmar (lo firma la Store) y sin instalar.
- **Abiertas a la vez:** cada copia nombra su candado de instancia única y su ventana residente con su paquete, así que una no le cede el paso a la otra. El atajo sí es de todo el sistema: por eso cada una tiene el suyo.
- **Iconos:** por ahora, el símbolo de Kata (`crates/ui/src/icon.rs`) en blanco sobre una placa azul, dibujado al empaquetar. En Kata Dev, la parte de abajo es una etiqueta blanca que dice «Dev», también en la bandeja, y sus ventanas se llaman «Kata Dev». La identidad definitiva está pendiente.

## Metas de rendimiento

| Medida | Meta | Medido |
|---|---|---|
| Memoria en reposo | < 15 MB | 10 MB (12 MB tras capturar) |
| CPU en reposo | 0 % | 0 % |
| Atajo → selección visible | < 100 ms | 30–40 ms |
| Aplicar un desenfoque | < 16 ms | 7.8 ms |
| Cuadro de la animación del Foco (captura 4K) | < 16 ms | 12 ms (peor caso) |
| Tamaño del ejecutable | < 5 MB | 0.3 MB |
| PNG frente al de Windows | Nunca mayor | 18 % menor de media en las pruebas; con fotos, hasta 0,5 % mayor |
