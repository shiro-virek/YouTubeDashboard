# YouTube Dashboard

A portable Linux desktop application to organize and open YouTube channels. Built with Rust, GTK 4, libadwaita, and an embedded SQLite database — everything you need is a single binary plus one `.db` file that can live next to the executable.

## Features

- **Manual channel management**: add channels with a name, URL, and optional tags.
- **Grid or list views**: switch between compact and spacious layouts.
- **Drag and drop reordering**: reorder visible channels by dragging their cards.
- **Smart filtering**: search by name (multi-term AND, case- and diacritic-insensitive). Filter by tags (OR semantics across selected tags).
- **Open in your browser**: click a channel to open it in your preferred browser, or configure a custom command in Preferences.
- **Double-click to open**: double-click any card to open its channel.
- **Context menu**: copy the URL, edit, or delete from each card.
- **Configurable browser**: choose system default, a detected browser, or a custom command (saved to `ytdash.conf` next to the database for portability).
- **Portable by design**: prefers `ytdash.db` next to the executable. Falls back to `$XDG_DATA_HOME/ytdash/` if the executable directory is not writable. Override with `--db` or `YTDASH_DB`. WAL is checkpointed on exit so the `.db` file stays self-contained.

## Requirements

Only GTK 4 and libadwaita system libraries are required. SQLite is embedded, so no separate `libsqlite3` installation is needed.

### Debian/Ubuntu
```sh
sudo apt install libgtk-4-dev libadwaita-1-dev build-essential pkg-config
```

### Fedora
```sh
sudo dnf install gtk4-devel libadwaita-devel gcc pkgconf-pkg-config
```

Prebuilt binaries only need GTK 4 and libadwaita installed on the target system.

## Building

```sh
cargo build --release
```

The binary will be at `target/release/ytdash`. For maximum portability, copy it alongside its `.db` and `.conf` files.

## Usage

```sh
ytdash                    # use portable database next to executable
ytdash --db /path/to.db   # use a specific database
ytdash --help             # show help
ytdash --version          # show version
```

### Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl+N` | Add channel |
| `Ctrl+F` | Focus search |
| `Ctrl+L` | Clear filters |
| `Ctrl+1` | Grid view |
| `Ctrl+2` | List view |
| `Ctrl+,` | Open Preferences |
| `Ctrl+Q` | Quit |

## Data location

`ytdash` resolves the database path in this order:

1. `--db` / `-d` / `--db=...`
2. `YTDASH_DB` environment variable
3. `ytdash.db` next to the executable (portable) **if the directory is writable**
4. `$XDG_DATA_HOME/ytdash/ytdash.db`

Preferences are stored in `ytdash.conf` in the same directory as the database, so both files travel together when copied to a USB stick.

To locate the current data folder, use **Open data folder** from the main menu.

## URL formats

Accepted and normalized automatically:

- `@handle`
- `youtube.com/@handle`
- `youtube.com/c/Name`
- `youtube.com/user/Name`
- `youtube.com/channel/UC...`
- `youtube.com/@handle/videos` (and other subpaths are preserved)

If you leave the name empty, it will be derived from the URL. The `/videos` suffix is trimmed from the handle label when appropriate.

Tags are comma-separated, case-insensitive, and automatically removed when no channel uses them.

## Development

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

Domain logic (`model.rs`), persistence (`db.rs`), and application state (`state.rs`) are decoupled from the UI (`ui/`) and covered by unit tests that run headlessly.

## Project structure

```text
src/
├── config.rs      Preferences and browser launch logic
├── main.rs        CLI entry point and application setup
├── model.rs       Domain: channels, URL normalization, filtering, tags, ordering
├── db.rs          SQLite schema, CRUD, portable DB path resolution
├── state.rs       In-memory app state and operations
└── ui/
    ├── mod.rs     Window, header, filters, rendering, actions
    ├── card.rs    Channel card, context menu, drag & drop
    ├── editor.rs  Add/edit channel dialog
    ├── prefs.rs   Preferences dialog (browser selection)
    ├── avatar.rs  Circular generated avatars
    └── styles.rs  Application CSS
```

## License

MIT
