# MacHunt

A fully local macOS search tool for files and folders, with both CLI and native GUI (Tauri + React). No HTTP backend service is required.

[中文文档](README_zh.md)

## Version

- GUI: `v0.2.4`
- CLI/Core: `v0.2.4`

## Latest Updates (v0.2.4)

- Replaced `qlmanage -p` preview with native macOS Quick Look (`QLPreviewPanel`) integration.
- Space preview now uses native Finder-style panel behavior with proper key-window focus and responder chain.
- Removed frontend synthetic preview zoom/status layer (`preview://status` + overlay animation), now relying on system-native transitions.

- Window close behavior changed to **hide to background** (`Cmd+W` / red close button), while `Cmd+Q` still quits.
- Hidden window now switches to macOS accessory mode, so it does **not appear in Cmd+Tab**.
- Added configurable global shortcut to toggle window visibility.
- Showing window by shortcut auto-focuses the search input.
- Settings page now supports vertical scrolling.
- Clicking blank area in result list clears current selection.
- Clipboard copy in bundled builds is now hardened with multi-path fallback.
- Settings now include **Launch at Login** and **Silent Startup** options.
- Launch-at-login now integrates with macOS **Login Items** (not only background items).
- Startup settings card now has complete dark-theme styling.
- Added **Excluded Directories** settings for index build/rebuild filtering.
- Exclusion rules support:
  - Exact directory paths (e.g. `/Volumes/`)
  - Regex or wildcard directory patterns (e.g. `*/.git/*`)
- Exact-directory mode now supports Finder path picker + manual add.
- Fixed wildcard exclusion semantics so `*/.git/*` correctly excludes any path segment containing `/.git/`.

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
  - Copy name / path
  - Copy all selected names / paths
  - Move to Trash
- Theme settings (system/light/dark)
- Language settings (zh/en)
- Global shortcut settings for show/hide window
- Startup settings: launch at login + silent startup (applies to auto-launch only)
- Excluded-directory settings:
  - Exact directory rules + regex/wildcard rules
  - Finder picker for exact rules
  - Rules are persisted and applied during build/rebuild

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
- `--logs`: write logs to `~/.machunt/logs`
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

- DB: `~/.machunt/data/index.db`
- GUI settings: `~/.machunt/gui/settings.json` (shortcut + launch-at-login + silent-start)
- Exclude directory rules: stored in `meta` table of `~/.machunt/data/index.db`
- Logs: `~/.machunt/logs/`

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
