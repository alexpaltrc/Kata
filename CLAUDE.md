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

## La app de uso diario: Kata Dev

Alex usa **Kata Dev**, la MSIX instalada. Nunca se abre el `Kata.exe` de `target/`.

- **Identidad propia:** `AlexPalacios.Kata.Dev`, «Kata Dev», icono naranja. Firmada con el certificado `CN=Alex Palacios Dev`, confiado solo en la PC de Alex. Sin modo de desarrollador.
- **Sus ajustes:** Windows le da a cada paquete su propia copia del registro y la conserva en cada actualización.
- **Actualizarla:** `packaging/update-dev.ps1`, el acceso directo «Actualizar Kata» del escritorio de Alex. Baja los cambios de GitHub, compila, instala encima y la abre.
- **Al terminar cualquier cambio en la PC de Alex,** córrelo con `-NoPull -NoPause` y di en una línea que ya está actualizada. Si cierras la app, vuelve a abrirla.

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
| `packaging/` | El paquete MSIX de Kata Dev y «Actualizar Kata» |
| `docs/` | Producto y arquitectura |

## Comandos

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p kata
powershell -ExecutionPolicy Bypass -File packaging\update-dev.ps1 -NoPull -NoPause
```

## Claude Code en la nube

La nube corre en Linux, así que no puede ejecutar Kata. Ahí:

- `cargo test -p kata-editor` funciona, porque el editor es Rust puro.
- `cargo check --workspace --target x86_64-pc-windows-msvc` revisa todo el código de Windows.
- La CI de GitHub (Windows) es la verificación final. No des un cambio por terminado si la CI falla.
- Al terminar, dile a Alex que lo traiga a su Kata Dev con «Actualizar Kata».
