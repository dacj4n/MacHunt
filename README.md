# MacHunt

A fully local macOS search tool for files and folders, with both CLI and native GUI (Tauri + React). No HTTP backend service is required.

[中文文档](README_zh.md)

## Version

- GUI: `v0.3.3`
- CLI/Core: `v0.3.3`

## Latest Updates (v0.3.3)

- Default global shortcut changed from `CmdOrCtrl+Shift+KeyF` to `CmdOrCtrl+Shift+KeyD`.
- Default exclude-pattern directories on fresh install:
  - `/System/**`, `/private/var/**`, `/private/tmp/**` — macOS system & private directories
  - `/.Spotlight-V100/**`, `/.fseventsd/**` — Spotlight & FSEvents metadata
  - `/dev/**`, `/proc/**` — device & process virtual filesystems

- Fixed multi-select trash action: when deleting via right-click context menu with multiple items selected, all selected items are now moved to trash instead of only the context-menu target.
- Improved centering of empty-state placeholder text ("Type a keyword to start searching." / "No matching files found.") in the results area by restructuring the table-shell grid layout.

- Added native macOS file-object copy from search results:
  - copy single or multiple results and paste directly in Finder/other locations
  - supports right-click `Copy Result` / `Copy All Results`
  - supports keyboard `Cmd/Ctrl + C` on selected results
  - implemented with native `NSPasteboard` (no AppleScript path)
- Refined global shortcut window toggle behavior:
  - if window is visible but not focused/topmost, shortcut now brings it to front first
  - only hides when the window is already visible and focused

- Fixed duplicate search results caused by macOS volume mirror paths (`/Volumes/System` and `/Volumes/Macintosh HD`).
- Upgraded index architecture to "continuous incremental first":
  - watcher supports configurable multi-root monitoring
  - build/watch now share unified exclude rule semantics
  - full build writes DB + in-memory index in a single pass (removed post-build full reload)
  - dirty-root partial reindex worker added
  - startup dead-path cleanup is now chunked background scan
- Added GUI settings for watch roots (add/remove + Finder picker + persistence).

## Core Capabilities

### CLI

- Build / rebuild local index
- Search by substring or wildcard pattern
- File-only / folder-only filtering
- Path-prefix filtering
- `watch` mode with FSEvents incremental updates
- `optimize --vacuum` for DB maintenance

### GUI

- Instant local search (no server)
- Regex toggle + case-sensitive toggle
- Path filter: manual input + suggestion dropdown + Finder picker
- Auto watcher start on app launch
- Build / rebuild controls
- Multi-tab category filter (all/files/folders/documents/images/media/code/archives)
- Click-to-sort on table headers (name/path/type/size/modified)
- Draggable column splitters with persisted widths
- Single/multi selection (`Shift` range, `Cmd` additive)
- Keyboard row navigation (`ArrowUp/ArrowDown`)
- Space-triggered Quick Look preview (multi-selection supported)
- Double-click to open files/folders
- Context menu actions:
  - Open
  - Open With... (Finder / QSpace Pro / Terminal / WezTerm)
  - Copy result / copy all selected results (native file-object clipboard)
  - Copy name / path
  - Copy all selected names / paths
  - Move to Trash
- `Cmd/Ctrl + C` to copy selected search results as file objects
- Theme settings (system/light/dark)
- Language settings (zh/en)
- Global shortcut settings for show/hide window
- Startup settings: launch at login + silent startup (applies to auto-launch only)
- Index maintenance settings: enable/disable auto `VACUUM` after rebuild
- Excluded-directory settings:
  - Exact directory rules + regex/wildcard rules
  - Finder picker for exact rules
  - Rules are persisted and applied during build/rebuild
- Watch-root settings:
  - Configure watcher roots instead of always monitoring `/`
  - Add/remove roots from GUI settings
  - Finder picker for root selection

## Tech Stack

- Core: Rust
- CLI: Clap
- GUI frontend: React 18 + TypeScript + Vite
- GUI container: Tauri 2
- Global shortcut: `tauri-plugin-global-shortcut`
- Storage: SQLite (`rusqlite`, WAL mode)
- In-memory index: `DashMap<String, Vec<PathBuf>>`
- Scanner: WalkDir + Crossbeam channels
- Watcher: macOS FSEvents (CoreServices)

## Architecture Overview

- `src/`: shared core engine used by both CLI and GUI
- `src-tauri/`: Tauri backend commands and window/menu lifecycle
- `src/App.tsx`: GUI application logic and interactions

## Build and Run

### Requirements

- macOS 10.15+
- Rust 1.70+
- Node.js 18+
- npm 9+

### Install

```bash
git clone https://github.com/dacj4n/MacHunt.git
cd MacHunt
```

### CLI

```bash
cargo build --release
./target/release/machunt --help
```

### GUI (dev)

```bash
npm install
npm run tauri dev
```

### GUI (package)

```bash
npm run build
npm run tauri build
```

## Build Command Difference

### `npm run build`

- Only builds frontend assets (TypeScript + Vite)
- Outputs static files to `dist/`
- Does **not** compile Rust backend
- Does **not** produce `.app` / `.dmg`

### `npm run tauri build`

- Builds the full desktop app package
- Runs `beforeBuildCommand` in `src-tauri/tauri.conf.json` first (currently `npm run build`)
- Compiles Rust code under `src-tauri/`
- Generates installable artifacts such as `.app` / `.dmg`

## App Icon Generation (Tauri)

Use the following command to generate multi-size app icons from a 1024x1024 source image:

```bash
npm run tauri icon src-tauri/icons/app-icon-1024.png
```

Notes:

- The source image should be square, preferably `1024x1024`.
- This command generates platform icon assets under `src-tauri/icons/`.
- After regeneration, `npm run tauri build` will bundle the updated icons automatically (as referenced by `src-tauri/tauri.conf.json`).

## CLI Reference

Syntax:

```bash
machunt [OPTIONS] [QUERY] [COMMAND]
```

Top-level options:

- `-p, --path <PATH>`: path prefix filter (`.` means none)
- `-r, --regex`: wildcard pattern mode
- `--folder`: folders only
- `--file`: files only
- `--logs`: write logs to `~/Library/Caches/MacHunt/logs`
- `[QUERY]`: search query when no subcommand is used

Selection rule:

- no `--file` and no `--folder` => include both files and folders
- only `--file` => files only
- only `--folder` => folders only

Subcommands:

### `build`

```bash
machunt build [OPTIONS]
```

- `-p, --path <PATH>`: build only this scope
- `--rebuild`: clear old index first
- `--include-dirs <true|false>`: include directories (default `true`)

### `watch`

```bash
machunt watch
```

- Starts FSEvents watcher
- Replays from last EventID when available
- Enters interactive search loop

### `optimize`

```bash
machunt optimize [--vacuum]
```

- Always runs WAL checkpoint
- Optional `--vacuum` to reclaim DB file space

Wildcard rules (`--regex`):

- `*` => any chars except `/`
- `**` => any chars including `/`
- `?` => one char except `/`
- `{a,b}` => `a` or `b`

## Runtime Data

- DB: `~/Library/Caches/MacHunt/index.db`
- GUI settings: `~/Library/Application Support/MacHunt/settings.json` (shortcut + launch-at-login + silent-start + show-dock-icon + auto-vacuum-on-rebuild + excluded-directory rules + watch roots)
- Exclude directory rules: stored in `settings.json` (`excludeExactDirs` / `excludePatternDirs`)
- Watch roots: stored in `settings.json` (`watchRoots`) and synced to DB meta (`watch_roots`)
- Logs: `~/Library/Caches/MacHunt/logs/`

## Why DB Can Be Large

Common reasons:

- Large file count (often millions)
- Directory indexing enabled by default
- Long path strings dominate storage
- `dirs + files` relational structure still stores large cardinality
- WAL files (`index.db-wal`) can temporarily become large during writes

Maintenance:

```bash
machunt optimize --vacuum
```

## License

MIT
