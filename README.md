# MacHunt

A macOS local file/folder search tool with both CLI and native GUI, fully local (no HTTP backend service required).

[中文文档](README_zh.md)

## Versions (2026-03-21)

- GUI: `v1.1.0`
- CLI / Core: `v0.2.1`

> Previous: GUI `v1.0.0`, CLI/Core `v0.2.0`

## What's New (v1.1.0 / v0.2.1)

### Added

- GUI now auto-starts watcher on app launch
- Result context menu:
  - Open
  - Open With ... (submenu: Reveal in Finder / View in QSpace Pro)
  - Copy Name / Copy Path
  - Move to Trash
- Double-click to open files/folders
- Path filter supports dropdown + manual input + Finder path picker
- Clickable sorting on all 4 headers with asc/desc indicator on active column only
- Column resize width persistence across restarts

### Fixed

- "Open With ..." submenu disappearing while moving mouse
- Small-window layout overlap/compression in result table
- Column resizer hit area too small / hard to drag
- Dark theme action controls lacked visible highlight feedback
- Input cursor style restored to text-edit cursor in input fields
- Duplicate indexing/result edge cases reduced by dedupe + incremental memory update path

### Optimized

- DB schema moved to `dirs + files` model (directory dedup)
- Removed legacy `idx_name` index to reduce storage overhead
- Path-scoped build (`build --path`) now updates memory index incrementally instead of full reload
- Startup validation cleanup for dead paths
- Search pipeline adds low-cost prefilter before heavier matching

## Tech Stack

- Core language: Rust
- CLI parsing: Clap
- GUI: Tauri 2 + React 18 + TypeScript + Vite
- Storage: SQLite (WAL)
- In-memory index: DashMap
- Scanning: WalkDir + Crossbeam
- Watching: macOS FSEvents (CoreServices)

## Architecture and Flow

### Layers

- `Core Engine` (Rust)
  - scanning, indexing, DB persistence, in-memory search, FSEvents incremental updates
- `CLI` (Rust)
  - calls the same core engine
- `GUI` (Tauri + React)
  - calls local Tauri commands directly
  - no REST API, no extra backend port

### Runtime flow

1. `build` scans file system and writes batches into SQLite
2. index is loaded/updated into `DashMap<String, Vec<PathBuf>>`
3. `search` runs against memory index and returns paths
4. `watch` consumes FSEvents and updates DB + memory incrementally

## Why DB Size Can Be Large

Even with optimizations, DB can still grow significantly at multi-million scale:

- very large file counts
- file/dir relationship and required indexes still consume space
- WAL file can temporarily grow under write-heavy workloads
- directory indexing is enabled by default
- long path/file-name distribution amplifies storage footprint

Maintenance command:

```bash
machunt optimize --vacuum
```

## Project Structure

- `src/`: CLI + core engine
  - `main.rs`: CLI entry
  - `engine.rs`: engine orchestration
  - `builder.rs`: scanning + index build
  - `search.rs`: search implementation
  - `watcher.rs`: FSEvents watcher
  - `db.rs`: SQLite persistence
- `src-tauri/`: GUI backend (Tauri commands)
- `src/App.tsx`: GUI frontend

## Requirements

- macOS 10.15+
- Rust 1.70+
- Node.js 18+
- npm 9+

## Install

```bash
git clone https://github.com/dacj4n/MacHunt.git
cd MacHunt
```

## Build and Run

### CLI

```bash
cargo build --release
./target/release/machunt --help
```

### GUI (development)

```bash
npm install
npm run tauri dev
```

### GUI (production package)

```bash
npm run build
npm run tauri build
```

## CLI Full Parameter Reference

CLI syntax:

```bash
machunt [OPTIONS] [QUERY] [COMMAND]
```

### Top-level options

| Option | Type | Default | Description |
|---|---|---:|---|
| `-p, --path <PATH>` | string | `.` | Path prefix filter for search. `.` means no prefix filter |
| `-r, --regex` | flag | `false` | Wildcard pattern mode (not full PCRE regex) |
| `--folder` | flag | `false` | Search folders only |
| `--file` | flag | `false` | Search files only |
| `--logs` | flag | `false` | Enable logs (`~/.machunt/logs`) |
| `[QUERY]` | string | `""` | Query when no subcommand is used |

Selection behavior:

- neither `--file` nor `--folder`: search both files and folders
- `--file` only: files only
- `--folder` only: folders only
- both enabled: both files and folders

### Subcommands

#### 1) `build`

```bash
machunt build [OPTIONS]
```

| Option | Type | Default | Description |
|---|---|---:|---|
| `-p, --path <PATH>` | string | none | Build index for one path scope |
| `--rebuild` | flag | `false` | Clear old index then rebuild |
| `--include-dirs <true\|false>` | bool | `true` | Include directory entries while indexing |

Examples:

```bash
machunt build
machunt build --path "/Volumes/Tools"
machunt build --rebuild
machunt build --include-dirs false
```

#### 2) `watch`

```bash
machunt watch
```

Behavior:

- starts FSEvents watcher
- enters interactive search loop in terminal
- persists last EventID on exit for incremental resume

Can be combined with top-level options:

```bash
machunt --file --path "/Users/dcj" --regex watch
```

#### 3) `optimize`

```bash
machunt optimize [OPTIONS]
```

| Option | Type | Default | Description |
|---|---|---:|---|
| `--vacuum` | flag | `false` | run VACUUM in addition to WAL checkpoint |

Examples:

```bash
machunt optimize
machunt optimize --vacuum
```

### Direct search examples

```bash
machunt "test"
machunt --path "/Volumes/Tools" "test"
machunt --file "app"
machunt --folder "java"
machunt --regex "*app*.*"
```

### Wildcard rules (`--regex`)

- `*`: any chars except `/`
- `**`: any chars including `/`
- `?`: one char except `/`
- `{a,b}`: `a` or `b`

## GUI Usage

### Basic workflow

1. Launch GUI (watcher auto-starts by default)
2. Type query in top input
3. Narrow scope with path filter (input/dropdown/Finder picker)
4. Choose `Substring` or `Wildcard`, enable case-sensitive if needed
5. Use result interactions: sort, resize columns, double-click, right-click actions

### GUI control semantics

| UI control | Backend semantics | Description |
|---|---|---|
| Search input | `query` | keyword |
| Path filter | `pathPrefix` | path prefix filter |
| Substring/Wildcard | `mode` | `Substring` / `Pattern` |
| `Aa` | `caseSensitive` | case-sensitive toggle |
| Build | `build_index(rebuild=false)` | regular/incremental build |
| Rebuild | `build_index(rebuild=true)` | full rebuild |
| Start/Stop Watch | `start_watch_auto` / `stop_watch` | watcher toggle |
| Tabs | files/dirs/type filter | file, folder, documents, etc. |
| Header click | `sortKey + asc/desc` | 4-column sorting with indicator |

### Result interactions

- double click row: open file/folder
- right click row:
  - Open
  - Open With ...
    - Reveal in Finder
    - View in QSpace Pro
  - Copy Name
  - Copy Path
  - Move to Trash
- draggable column splitters with persisted widths
- ellipsis + hover tooltip for truncated text

### Settings

- Theme: Light / Dark / Follow System
- macOS menu item is fixed to `Preferences`

## Tauri Commands (for contributors)

| Command | Params | Purpose |
|---|---|---|
| `initialize` | - | load index on startup |
| `search` | `SearchRequest` | execute search |
| `build_index` | `path?, rebuild, include_dirs?` | build index |
| `start_watch_auto` | - | auto-start watcher strategy |
| `stop_watch` | - | stop watcher |
| `watch_status` | - | watcher status |
| `list_path_suggestions` | - | path suggestions |
| `pick_path_in_finder` | - | Finder folder picker |
| `open_search_result` | `path` | open file/folder |
| `reveal_in_finder` | `path` | reveal in Finder |
| `open_in_qspace` | `path` | open in QSpace Pro |
| `copy_to_clipboard` | `text` | copy text |
| `move_to_trash` | `path` | move item to Trash |
| `set_menu_language` | `language` | menu sync |
| `persist_watch_cursor` | - | persist EventID |

`SearchRequest` fields:

- `query: string`
- `mode: "Substring" | "Pattern"`
- `caseSensitive?: boolean`
- `pathPrefix?: string`
- `includeFiles?: boolean`
- `includeDirs?: boolean`
- `limit?: number` (GUI request currently defaults to 2500)

## Data and Logs

- DB: `~/.machunt/data/index.db`
- Logs: `~/.machunt/logs/`
- GUI column width persistence key: `machunt.table.column.widths`

## Default Skipped Paths (build/watch)

- `/dev`
- `/proc`
- `/sys`
- `/private/var/vm`
- `/private/var/run`
- `/private/var/folders`
- `/System/Volumes/Data`
- `/System/Volumes/Preboot`
- `/System/Volumes/Recovery`
- `/System/Volumes/VM`
- any path containing `/.Spotlight-V100` or `/.fseventsd`

## Permissions (macOS)

For full-disk coverage, grant **Full Disk Access**:

1. System Settings → Privacy & Security → Full Disk Access
2. authorize your terminal app for CLI
3. authorize MacHunt app for GUI
4. restart affected apps after authorization

## FAQ

### Why are some paths not searchable?

- path may be in default skip rules
- Full Disk Access not granted
- index has not been built for that scope yet

### Why can I still feel duplicate results sometimes?

Try rebuilding and vacuuming:

```bash
machunt build --rebuild
machunt optimize --vacuum
```

### Why can build/rebuild look "stuck"?

- large-scale DB writes + memory updates can take time
- first run in watch mode may trigger background bootstrap build

## License

MIT
