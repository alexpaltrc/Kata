# Arquitectura

## Interfaz: controles propios sobre Direct2D

Kata dibuja sus propios controles con el estilo Fluent de Windows 11, sobre Direct2D y DirectComposition, mediante la crate oficial `windows` de Microsoft.

**Por qué:**

- **No hay una opción oficial:** WinUI 3, la interfaz nativa de Microsoft, no tiene soporte real para Rust.
- **Son pocos controles:** Kata necesita unos seis (botón, slider por niveles, interruptor, selector de atajo, barra de herramientas y lienzo), así que hacerlos es abarcable.
- **Rendimiento:** no carga ningún framework, por lo que arranca al instante y gasta muy poca memoria.
- **Control total del diseño:** cada animación, sombra y transición se ajusta al detalle. Las animaciones las ejecuta el compositor de Windows, así que siguen fluidas aunque la app esté ocupada.
- **Mismo motor que el editor:** los efectos de desenfoque y foco también usan Direct2D, en la tarjeta gráfica.

**Descartado:** Tauri u otras opciones basadas en WebView (más pesadas y no nativas) y Slint (otra capa entre Kata y Windows).

## Captura

- La pantalla se congela al iniciar el recorte. Todos los monitores, con su escala propia.
- Las pantallas HDR se convierten a colores estándar para que la captura se vea igual en cualquier lugar.

## Guardado

- **Formato:** PNG, con el codificador nativo de Windows (WIC).
- **Sin bloqueos:** se guarda en segundo plano, así la app nunca se traba.
- **Escritura atómica:** primero un archivo temporal, luego se reemplaza. Nunca queda una imagen a medias.

## Sonido

- Un WAV de unos 80 ms dentro del ejecutable.
- Ya está cargado en memoria, así que suena sin retraso.

## Distribución

- **Paquete:** MSIX para x64 y ARM64, con `runFullTrust` (bandeja y atajo global) y `StartupTask` (iniciar con Windows).
- **Sin instalaciones extra:** el runtime de C va incluido dentro del ejecutable.

## Metas de rendimiento

| Medida | Meta |
|---|---|
| Memoria en reposo | < 15 MB |
| CPU en reposo | 0 % |
| Atajo → selección visible | < 100 ms |
| Tamaño del ejecutable | < 5 MB |
