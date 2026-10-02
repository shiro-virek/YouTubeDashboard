# ytdash — panel de canales de YouTube

Aplicación de escritorio para Linux, portable y sin dependencias de Node.js,
Python ni Java. Gestiona una lista personal de canales de YouTube en un único
archivo SQLite y abre cada canal en tu navegador con un clic.

## Características

- **Alta manual** de canales con nombre, URL y etiquetas, desde el botón
  «+» de la cabecera o con `Ctrl+N`.
- **Vista de cuadrícula o de lista**, conmutable desde la cabecera.
- **Reordenar arrastrando y soltando** las tarjetas.
- **Filtrar por nombre** (varios términos, todos deben coincidir).
- **Filtrar por etiquetas** mediante las chips de la barra superior.
- **Clic en un canal para abrirlo** en el navegador del sistema.
- **Copiar URL**, editar y eliminar desde el menú contextual de cada tarjeta.
- Menú de Filters, Atajos, Ayuda y Acerca de con las acciones habituales.

## Requisitos

Solo las bibliotecas de GTK 4 y libadwaita del sistema. **SQLite va
embebido**, así que no hace falta instalar `libsqlite3` ni nada más.

En Debian/Ubuntu:

```sh
sudo apt install libgtk-4-dev libadwaita-1-dev build-essential pkg-config
```

En Fedora:

```sh
sudo dnf install gtk4-devel libadwaita-devel gcc pkgconf-pkg-config
```

Para ejecutar el binario ya compilado basta con tener GTK 4 y libadwaita
presentes en el sistema.

## Compilar

```sh
cargo build --release
```

El ejecutable queda en `target/release/ytdash`. Se genera un único binario:
cópialo junto con su archivo `.db` y la aplicación es portable.

## Uso

```sh
ytdash                    # usa la base de datos portátil
ytdash --db rutas/mis.db  # usa una base de datos concreta
ytdash --help
```

Atajos de teclado:

| Atajo          | Acción                    |
| -------------- | ------------------------- |
| `Ctrl+N`       | Añadir canal              |
| `Ctrl+F`       | Buscar                    |
| `Ctrl+L`       | Limpiar filtros           |
| `Ctrl+1`       | Vista de cuadrícula       |
| `Ctrl+2`       | Vista de lista            |
| `Ctrl+Q`       | Salir                     |

## Dónde se guarda la base de datos

`ytdash` resuelve la ruta en este orden:

1. El valor de `--db` / `-d` / `--db=...`.
2. La variable de entorno `YTDASH_DB`.
3. `ytdash.db` junto al ejecutable —comportamiento portable— **si el
   directorio permite escritura**.
4. `$XDG_DATA_HOME/ytdash/ytdash.db` como alternativa.

Al cerrar, la aplicación hace un *checkpoint* del WAL para que el archivo
`.db` quede completo y se pueda copiar en otro equipo o en un pendrive sin
perder datos.

Para localizar el archivo exacto, usa la acción **Abrir carpeta de datos**
del menú principal.

## Formato de los enlaces

Se aceptan y normalizan automáticamente:

- `@handle`
- `youtube.com/@handle`
- `youtube.com/c/Name`
- `youtube.com/channel/UC…`
- `youtube.com/user/name`
- `youtube.com/@handle/videos`

Si escribes solo la URL y dejas el nombre vacío, se genera uno a partir del
enlace. El prefijo `@handle/videos` se recorta al manejador.

Las etiquetas se separan por comas; no distinguen mayúsculas de minúsculas y
se eliminan automáticamente cuando ningún canal las usa.

## Desarrollo

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

La lógica de dominio (`model.rs`), la persistencia (`db.rs`) y el estado de la
aplicación (`state.rs`) están separados de la interfaz (`ui/`) y cubiertos por
tests unitarios que no necesitan pantalla.

## Estructura

```
src/
├── main.rs        punto de entrada y argumentos de línea de comandos
├── model.rs       dominio: canales, URLs, búsqueda, etiquetas y orden
├── db.rs          esquema y acceso SQLite, resolución de ruta portable
├── state.rs       estado en memoria y operaciones de la aplicación
└── ui/
    ├── mod.rs     ventana, cabecera, filtros, render y acciones
    ├── card.rs    tarjeta de canal, menú contextual y arrastrar y soltar
    ├── editor.rs  diálogo de alta y edición
    ├── avatar.rs  avatares circulares generados a partir del nombre
    └── styles.rs  CSS de la aplicación
```

## Licencia

GPL-3.0-or-later.