# Kata

App de Windows 11, escrita en Rust, para tomar capturas de pantalla destinadas a manuales, guías e instructivos, y editarlas con dos herramientas: **Desenfocar** y **Foco**. Se distribuye como MSIX en la Microsoft Store.

Detalle del producto: [docs/producto.md](docs/producto.md). Decisiones técnicas: [docs/arquitectura.md](docs/arquitectura.md).

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
| `docs/` | Producto y arquitectura |

## Comandos

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p kata
```

## Claude Code en la nube

La nube corre en Linux, así que no puede ejecutar Kata. Ahí:

- `cargo test -p kata-editor` funciona, porque el editor es Rust puro.
- `cargo check --workspace --target x86_64-pc-windows-msvc` revisa todo el código de Windows.
- La CI de GitHub (Windows) es la verificación final. No des un cambio por terminado si la CI falla.
