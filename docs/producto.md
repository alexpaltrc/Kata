# Producto

## Capturar

- **Cómo empezar:** con el atajo (`Ctrl + Mayús + S` por defecto, configurable), con el icono de la bandeja o con el botón **Nuevo recorte** de la ventana principal.
- **Selección:** la pantalla se congela y se oscurece un poco. Al pasar el cursor se ilumina la ventana que está debajo (o el monitor, sobre el escritorio); un clic la captura. Arrastrar elige cualquier área. Una etiqueta muestra el tamaño en píxeles. `Esc` o clic derecho cancelan.
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
| Abrir el editor al capturar | Activado | Apagado, la captura solo se copia y se guarda. |
| Sonido | Activado | Al encenderlo suena, para oírlo. |
| Carpeta de guardado | `Imágenes\Kata` | **Cambiar** abre el selector de carpetas de Windows. |
| Iniciar con Windows | Desactivado | Kata arranca en la bandeja, sin abrir ventanas. |

## Próximo

Planeado para la siguiente versión. Al terminarse, cada punto pasa a su sección.

### Ventanas completas, aunque otra las tape

Al pasar el cursor por una ventana tapada en parte, su parte oculta aparece en la selección: la ventana "sube" al frente y se captura completa. También la parte que sale de la pantalla.

- **Cómo:** `PrintWindow` con `PW_RENDERFULLCONTENT` pide la ventana a Windows aunque esté tapada. Medido en la PC de Alex: 13–23 ms por ventana (Apple Music, Claude, ambas tapadas), completas.
- **Solo cuando hace falta:** si ninguna ventana de encima la toca, todo sigue igual (GDI). Si sí, se pide al pasar el cursor, en un hilo aparte, y se guarda por si se hace clic. Una ventana que no responde no traba a Kata: se queda lo congelado.
- **Bordes:** sobre el recorte congelado se pinta la ventana dentro de su rectángulo redondeado. Las esquinas quedan como se ven.
- **Por revisar:** ventanas de administrador, juegos y video protegido. Si `PrintWindow` falla, se prueba Windows.Graphics.Capture (sin borde amarillo: capacidad `graphicsCaptureWithoutBorder`) o se queda lo congelado.
- **Código:** `crates/capture` guarda el `HWND` de cada ventana; `overlay.rs` muestra y elige la ventana completa; `main.rs` recorta con ella.

### Kata como la app de capturas de Windows

Un ajuste, «Usar Kata con Impr Pant y Win + Mayús + S», hace que esas teclas abran el recorte de Kata en vez de Recortes.

- **Impr Pant:** en Windows 11 26H2 (build 26300, el de Alex) las apps ya pueden tomarla: la directiva «Make Print Screen key yieldable», sin configurar, lo permite. Kata ya acepta Impr Pant como atajo; hay que confirmar que se registra con Recortes instalado.
- **Win + Mayús + S:** Windows abre el protocolo `ms-screenclip:` (hoy asociado a Recortes). Plan A: Kata lo declara en su manifiesto y el ajuste lleva a Configuración › Aplicaciones predeterminadas, donde el usuario elige Kata (Windows no deja que una app se ponga sola como predeterminada). Plan B, si Windows no deja declarar `ms-screenclip`: un gancho de teclado de bajo nivel que solo mira esa combinación.
- **Rapidez:** abrir por protocolo arranca un segundo `Kata.exe` que avisa al que ya corre. Se mide contra la meta de 100 ms.
- **Por decidir con Alex:** si Alt + Impr Pant y Win + Impr Pant también pasan a Kata, y qué hacen.

## Fuera de alcance por ahora

- **Historial de capturas.** La app no guarda ningún registro de capturas. Se puede evaluar en el futuro.
