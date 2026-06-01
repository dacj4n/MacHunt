# MacHunt

A fully local macOS search tool for files and folders, with both CLI and native GUI (Tauri + React). No HTTP backend service is required.

[中文文档](README_zh.md)

## Version

- GUI: `v0.4.1`
- CLI/Core: `v0.4.1`

## Latest Updates (v0.4.1)

- **Major performance overhaul**: Replaced in-memory `DashMap` index (which stored
  full paths for every file) with SQLite FTS5 trigram search. Memory usage dropped from
  ~1.0 GB to ~200 MB, CPU from 96% to ~30%. The dirty-root polling worker was
  replaced with an event-driven design, enabling App Nap and near-zero idle CPU.
- **CLI `search` subcommand**: `machunt search "keyword"` for substring search,
  `-p "*.rs"` for wildcard patterns, `--json` for structured output, `-P` for path
  filtering, `-n` for result limit.
- **Fuzzy search**: `machunt search "redme" -F` finds "README" with typo tolerance
  (Levenshtein edit distance). Also available in GUI via regex+pattern mode.
- **APFS rename handling**: Fixed stale index entries when files are renamed on APFS.
  The watcher now cleans up the old path when only a RENAMED event is received.
- **Wildcard consistency**: Single `*` now consistently means "within one directory"
  in both search patterns and exclude rules (`**` for cross-directory).
- **CFString memory leak**: Fixed a Core Foundation reference leak in the FSEvents
  stream setup.
- **SQLite pragma tuning**: `mmap_size` 256→32 MB, `cache_size` 64→8 MB, reducing
  backend memory while maintaining sub-millisecond search latency.

## v0.4.0 Changes

- Dock icon redesign with Accessory mode and method swizzling.
- Data directories migrated to macOS-standard paths.
- Build/rebuild unified with temp DB + atomic rename (~7.5s for 3M files).
- Settings button in toolbar, focus restoration, i18n status bar.
- Configurable watch roots, unified exclude rules, dirty-root reindex worker.

## File Search Comparison

| | MacHunt | macOS Spotlight | Raycast | uTools |
|---|---|---|---|---|
| **Full filesystem scan** | Yes (~10s / 3M files) | Yes (via `mdfind`) | File search plugin | Plugin-based |
| **Search latency** | <5ms (FTS5 trigram) | 50–200ms+ | Varies by plugin | Varies by plugin |
| **Index format** | SQLite FTS5 (open, inspectable) | Proprietary | Proprietary | N/A |
| **CLI** | Yes (`machunt search`) | Yes (`mdfind`) | No | No |
| **Native GUI** | Yes (Tauri) | Built-in | Yes (Electron) | Yes (Electron) |
| **Fuzzy search** | Yes (Levenshtein) | Partial | No | No |
| **Incremental updates** | FSEvents | FSEvents | Varies | N/A |
| **Exclude / watch config** | Yes (GUI + wildcard) | Limited (System Prefs) | No | No |
| **Dock optional** | Yes (toggle) | N/A | No | No |
| **Open source** | Yes | No | No | Partially |

**Why MacHunt:**

- **Fast search** — SQLite FTS5 trigram index with sub-5ms latency for typical queries. No in-memory index needed.
- **Low footprint** — ~200 MB total (backend ~70 MB + WebView). App Nap enabled, near-zero idle CPU.
- **Full CLI** — `machunt search` with substring, wildcard, fuzzy, and JSON output modes.
- **Transparent configuration** — standard SQLite database, open and inspectable. Plain JSON settings.
- **Open source** — no proprietary services, fully auditable, build from source.

## Core Capabilities

### CLI

- Build / rebuild local index
- `search` subcommand with substring, wildcard, fuzzy, and JSON output
- Substring search (`machunt search "keyword"`)
- Wildcard pattern search (`machunt search -p "*.rs"`)
- Fuzzy/typo-tolerant search (`machunt search -F "redme"`)
- JSON output (`machunt search --json "keyword"`)
- File-only / folder-only filtering (`-f` / `-d`)
- Path-prefix filtering (`-P ~/projects`)
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
- Storage: SQLite FTS5 (`rusqlite`, WAL mode, trigram tokenizer)
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

## CLI Reference

Syntax:

```bash
machunt <COMMAND>
```

Subcommands:

### `search`

```bash
machunt search [OPTIONS] <QUERY>
```

- `<QUERY>`: search keyword
- `-p, --pattern`: wildcard/regex mode (`*.rs`, `test?.txt`)
- `-F, --fuzzy`: fuzzy/typo-tolerant search (Levenshtein distance)
- `-c, --case-sensitive`: case-sensitive search
- `-n, --limit <N>`: max results (default 100)
- `-P, --path <PATH>`: path prefix filter
- `-f, --files`: files only
- `-d, --dirs`: directories only
- `--json`: JSON output

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

- Starts FSEvents watcher with incremental updates
- Replays from last EventID when available
- Enters interactive search loop

### `optimize`

```bash
machunt optimize [--vacuum]
```

- Always runs WAL checkpoint
- Optional `--vacuum` to reclaim DB file space

Wildcard rules (`--pattern`):

- `*` => any chars except `/` (single directory level)
- `**` => any chars including `/` (all levels)
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
