# Arquitectura

## Interfaz: controles propios sobre Direct2D

Kata dibuja sus propios controles con el estilo Fluent de Windows 11, sobre Direct2D y DirectComposition, mediante la crate oficial `windows` de Microsoft.

**Por qué:**

- **No hay una opción oficial:** WinUI 3, la interfaz nativa de Microsoft, no tiene soporte real para Rust.
- **Son pocos controles:** Kata necesita unos seis (botón, slider por niveles, interruptor, selector de atajo, barra de herramientas y lienzo), así que hacerlos es abarcable.
- **Rendimiento:** no carga ningún framework, por lo que arranca al instante y gasta muy poca memoria.
- **Control total del diseño:** cada animación, sombra y transición se ajusta al detalle. Las animaciones las ejecuta el compositor de Windows, así que siguen fluidas aunque la app esté ocupada.

**Descartado:** Tauri u otras opciones basadas en WebView (más pesadas y no nativas) y Slint (otra capa entre Kata y Windows).

## Captura

- La pantalla se congela al iniciar el recorte. Todos los monitores, con su escala propia.
- **Hoy:** GDI copia todo el escritorio de una vez. En el mismo instante se anotan las ventanas y monitores, para resaltarlos al pasar el cursor.
- **Pendiente (fase 5):** convertir las pantallas HDR a colores estándar, para que la captura se vea igual en cualquier lugar.

## Editor

- **Efectos en el procesador:** el desenfoque y el foco se calculan con Rust puro (`crates/editor`). Lo que se ve en pantalla es exactamente lo que se guarda y se copia, y se puede probar sin Windows.
- **Solo lo que cambia:** tras cada edición se recalcula y se envía a la tarjeta gráfica únicamente la zona afectada.
- **Entrega:** la captura se copia y se guarda en cuanto se abre el editor. Cada cambio, al terminar, se copia de nuevo (y, con Autoguardado, se guarda). Al cerrar, se entrega lo último.
- **Deshacer:** el historial vive solo en memoria, mientras el editor está abierto.

## Guardado

- **Formato:** PNG, con el codificador nativo de Windows (WIC).
- **Sin bloqueos:** se guarda en segundo plano, así la app nunca se traba.
- **Escritura atómica:** primero un archivo temporal, luego se reemplaza. Nunca queda una imagen a medias.
- **Portapapeles:** se copia al instante, antes de guardar.

## Sonido

- Un toque suave de unos 120 ms, generado por código al arrancar. No hay archivos de audio.
- Ya está en memoria, así que suena sin retraso.

## Distribución

- **Paquete:** MSIX para x64 y ARM64, con `runFullTrust` (bandeja y atajo global) y `StartupTask` (iniciar con Windows).
- **Sin instalaciones extra:** el runtime de C va incluido dentro del ejecutable.

## Metas de rendimiento

| Medida | Meta | Medido |
|---|---|---|
| Memoria en reposo | < 15 MB | Pendiente |
| CPU en reposo | 0 % | Pendiente |
| Atajo → selección visible | < 100 ms | Pendiente |
| Aplicar un desenfoque | < 16 ms | 7.8 ms |
| Cuadro de la animación del Foco (captura 4K) | < 16 ms | 12 ms (peor caso) |
| Tamaño del ejecutable | < 5 MB | 0.3 MB |
