# Kata

App de Windows 11, escrita en Rust, para tomar capturas de pantalla destinadas a manuales, guías e instructivos, y editarlas con dos herramientas: **Desenfocar** y **Foco**. Se distribuye como MSIX en la Microsoft Store.

Detalle del producto: [docs/producto.md](docs/producto.md). Decisiones técnicas: [docs/arquitectura.md](docs/arquitectura.md). Guía para quien la usa: [docs/guia.md](docs/guia.md). La compresión PNG propia: [docs/compresion.md](docs/compresion.md).

Sigue siempre el estándar personal de Alex (skill `alex-vibecode`), en cada conversación, en la PC y en la nube, aunque nadie lo mencione. Manda su versión más reciente. La nube no ve ese skill, así que lo esencial va aquí.

## Prioridades (en este orden)

1. **Ultraminimalista.** Siempre, sin importar qué.
2. **Ultra simple.** Pocas opciones y nada que haya que leer para entenderlo. Todo sigue un orden natural.
3. **Moderno.** Tecnologías, diseño y animaciones actuales.
4. **Nativo.** El lenguaje visual más reciente de Windows 11: Mica, Fluent, Segoe UI Variable.
5. **Rendimiento.** Excelente en cualquier equipo, monitor y hardware. Si no está optimizado, no funciona.
6. **Elegancia.** Cada animación, pantalla e interacción es sofisticada. Esto incluye el código y el repositorio.

Ante cualquier duda, gana la opción más simple.

## Cómo comunicarse con Alex

- Resume y sé conciso.
- Explica todo para que lo entienda alguien no técnico.
- Al cerrar cada fase o cambio, di exactamente qué probar.

## Kata y Kata Dev

Alex usa a diario **Kata**, la versión estable, con sus ajustes reales. Cada cambio se hace y se prueba en **Kata Dev**, que solo usa datos de prueba. Ninguna se abre desde `target/`.

- **Separadas por completo:** `AlexPalacios.Kata` («Kata») y `AlexPalacios.Kata.Dev` («Kata Dev»). Las dos pueden estar abiertas a la vez. Cada una tiene sus propios ajustes, y Kata Dev además su propio atajo (`Ctrl + Mayús + D`) y su propia carpeta (`Imágenes\Kata Dev`). Kata Dev se reconoce por la etiqueta «Dev» de su icono (también en la bandeja) y por el nombre de sus ventanas. Son MSIX firmadas con el certificado `CN=Alex Palacios Dev`, confiado solo en la PC de Alex. Sin modo de desarrollador.
- **Al terminar cualquier cambio en la PC de Alex,** actualiza Kata Dev con `packaging/update-dev.ps1 -NoPull` y di en una línea que ya está. Si la cierras, vuelve a abrirla.
- **La estable no se toca mientras se trabaja.** Solo cambia cuando Alex decide lanzar una versión: se sube el número en `Cargo.toml`, se instala con `packaging/build-msix.ps1 -Stable` y se vuelve a abrir si estaba abierta.
- **Bajar y subir:** cada conversación, la de la PC y la de la nube, empieza bajando lo último de GitHub y termina subiendo su trabajo. Al volver a la de la PC, antes de nada: baja lo último, une a `main` el trabajo de la nube que esté en otra rama (si su CI pasó), actualiza Kata Dev con `packaging/update-dev.ps1` y di qué cambios llegaron.

## Convenciones

- **Idioma:** código, comentarios y commits en inglés. Documentación en español. Los textos de la app, en español e inglés.
- **Autoría:** Alex Palacios. Nunca se pone un correo en los archivos del proyecto.
- **Repositorio público:** a diferencia del estándar, porque aloja la política de privacidad que enlaza la Store. Es solo para Kata.
- **Licencia:** aún sin definir. No agregar ninguna hasta que Alex lo decida.
- **Documentación al día:** cada cambio actualiza los documentos de `docs/` que toca. Cada experimento de compresión, funcione o no, se anota en `docs/compresion.md`.
- **Repositorio limpio:** sin archivos sobrantes, carpetas vacías ni código muerto. Cada archivo tiene una razón de existir.
- **Todo es nuestro:** nada de librerías ni código de terceros. Todo se escribe desde cero, a partir de las especificaciones públicas. La API de Windows, mediante las crates oficiales `windows` de Microsoft, sí se usa.
- **Rendimiento:** se mide, no se supone.

## Estructura

| Ruta | Contenido |
|---|---|
| `crates/app` | Binario `Kata.exe`: arranque, bandeja, atajo global, ventanas |
| `crates/capture` | Captura multi-monitor (DPI mixto, HDR) |
| `crates/editor` | Modelo de edición: herramientas, deshacer, guardado. Rust puro, sin APIs de Windows |
| `crates/ui` | Controles Fluent, animaciones y tema (Direct2D y DirectComposition) |
| `packaging/` | Los paquetes MSIX de Kata y Kata Dev |
| `docs/` | Producto, arquitectura, guía, compresión y la política de privacidad que enlaza la Store |

## Comandos

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p kata
powershell -ExecutionPolicy Bypass -File packaging\update-dev.ps1 -NoPull
```

## Claude Code en la nube

La nube corre en Linux, así que no puede ejecutar Kata. Ahí:

- `cargo test -p kata-editor` funciona, porque el editor es Rust puro.
- `cargo check --workspace --target x86_64-pc-windows-msvc` revisa todo el código de Windows.
- La CI de GitHub (Windows) es la verificación final. No des un cambio por terminado si la CI falla.
- Al terminar, sube tu trabajo a GitHub. La conversación de la PC lo trae a Kata Dev.
