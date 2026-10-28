# Kata

App de Windows 11, escrita en Rust, para tomar capturas de pantalla destinadas a manuales, guías e instructivos, y editarlas con dos herramientas: **Desenfocar** y **Foco**. Se distribuye como MSIX en la Microsoft Store.

Detalle del producto: [docs/producto.md](docs/producto.md). Decisiones técnicas: [docs/arquitectura.md](docs/arquitectura.md).

Sigue el estándar personal de Alex (skill `alex-vibecode`). La nube no ve ese skill, así que lo esencial va aquí.

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

Alex usa a diario **Kata**, la versión estable. Cada cambio se hace y se prueba en **Kata Dev**. Ninguna se abre desde `target/`. La primera versión estable sale al terminar la fase 5; hasta entonces, solo existe Kata Dev.

- **Separadas por completo:** `AlexPalacios.Kata` («Kata», icono azul) y `AlexPalacios.Kata.Dev` («Kata Dev», icono naranja). Las dos pueden estar abiertas a la vez, y cada una tiene sus propios ajustes. Son MSIX firmadas con el certificado `CN=Alex Palacios Dev`, confiado solo en la PC de Alex. Sin modo de desarrollador.
- **Al terminar cualquier cambio en la PC de Alex,** actualiza Kata Dev con `packaging/update-dev.ps1 -NoPull` y di en una línea que ya está. Si la cierras, vuelve a abrirla.
- **La estable no se toca mientras se trabaja.** Solo cambia cuando Alex decide lanzar una versión: se sube el número en `Cargo.toml`, se instala con `packaging/build-msix.ps1 -Stable` y se vuelve a abrir si estaba abierta.
- **Bajar y subir:** cada conversación, la de la PC y la de la nube, empieza bajando lo último de GitHub y termina subiendo su trabajo. Al volver a la de la PC, antes de nada: baja lo último, une a `main` el trabajo de la nube que esté en otra rama (si su CI pasó), actualiza Kata Dev con `packaging/update-dev.ps1` y di qué cambios llegaron.

## Convenciones

- **Idioma:** código, comentarios y commits en inglés. Documentación en español. Los textos de la app, en español e inglés.
- **Autoría:** Alex Palacios. Nunca se pone un correo en los archivos del proyecto.
- **Licencia:** aún sin definir. No agregar ninguna hasta que Alex lo decida.
- **Repositorio limpio:** sin archivos sobrantes, carpetas vacías ni código muerto. Cada archivo tiene una razón de existir.
- **Dependencias:** solo si se justifican. Antes que una crate externa, preferir la API de Windows.
- **Rendimiento:** se mide, no se supone.

## Estructura

| Ruta | Contenido |
|---|---|
| `crates/app` | Binario `Kata.exe`: arranque, bandeja, atajo global, ventanas |
| `crates/capture` | Captura multi-monitor (DPI mixto, HDR) |
| `crates/editor` | Modelo de edición: herramientas, deshacer, guardado. Rust puro, sin APIs de Windows |
| `crates/ui` | Controles Fluent, animaciones y tema (Direct2D y DirectComposition) |
| `packaging/` | Los paquetes MSIX de Kata y Kata Dev |
| `docs/` | Producto y arquitectura |

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
