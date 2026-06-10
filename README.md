<h1 align="center">MacHunt</h1>

<p align="center"><img src="src-tauri/icons/icon.png" width="384" alt="MacHunt Icon" /></p>

<p align="center">
  <img src="https://img.shields.io/badge/-Rust-000000?logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/-TypeScript-3178C6?logo=typescript&logoColor=white" alt="TypeScript" />
  <img src="https://img.shields.io/badge/-CSS-1572B6?logo=css3&logoColor=white" alt="CSS" />
  <img src="https://img.shields.io/badge/-Objective--C-3A95E3?logo=apple&logoColor=white" alt="Objective-C" />
  <img src="https://img.shields.io/badge/-HTML-E34F26?logo=html5&logoColor=white" alt="HTML" />
  <img src="https://img.shields.io/badge/-React-61DAFB?logo=react&logoColor=black" alt="React" />
  <img src="https://img.shields.io/badge/-Tauri-24C8D8?logo=tauri&logoColor=white" alt="Tauri" />
</p>

A fully local macOS file/folder search tool with both CLI and native GUI (Tauri + React). No HTTP backend, no cloud services.

[中文文档](README_zh.md)

## Introduction

MacHunt scans your entire filesystem into a local SQLite FTS5 index. CLI searches complete in <5ms. It uses macOS FSEvents for incremental live updates. Think Spotlight, but fully open source, with a powerful CLI, and your data never leaves your machine.

## Screenshots

<table>
<thead>
<tr>
<th width="50%" align="center">Search</th>
<th width="50%" align="center">Pinned</th>
</tr>
</thead>
<tbody>
<tr>
<td align="center"><a target="_blank" rel="noopener noreferrer" href="./screenshots/search.png"><img src="./screenshots/search.png" alt="Search" width="100%" style="max-width: 100%;"></a></td>
<td align="center"><a target="_blank" rel="noopener noreferrer" href="./screenshots/pinned.png"><img src="./screenshots/pinned.png" alt="Pinned" width="100%" style="max-width: 100%;"></a></td>
</tr>
<tr>
<td align="center"><strong>Full-disk search, category tabs</strong></td>
<td align="center"><strong>Pinned favorites, persistent across restarts</strong></td>
</tr>
</tbody>
</table>

<table>
<thead>
<tr>
<th width="50%" align="center">Quick Look Preview</th>
<th width="50%" align="center">Path Filter & Context Menu</th>
</tr>
</thead>
<tbody>
<tr>
<td align="center"><a target="_blank" rel="noopener noreferrer" href="./screenshots/quicklook.png"><img src="./screenshots/quicklook.png" alt="Quick Look" width="100%" style="max-width: 100%;"></a></td>
<td align="center"><a target="_blank" rel="noopener noreferrer" href="./screenshots/context-menu.png"><img src="./screenshots/context-menu.png" alt="Context Menu" width="100%" style="max-width: 100%;"></a></td>
</tr>
<tr>
<td align="center"><strong>Space-triggered native Quick Look</strong></td>
<td align="center"><strong>Finder picker, right-click actions</strong></td>
</tr>
</tbody>
</table>

## Install

Download the latest `.dmg` from [GitHub Releases](https://github.com/dacj4n/MacHunt/releases), mount it, and drag `MacHunt.app` to `/Applications`.

> **First launch**: macOS Gatekeeper may block unsigned apps. If you see "cannot be verified", right-click `MacHunt.app` in Finder and select **Open**, then click **Open** in the dialog. Or run `xattr -cr /Applications/MacHunt.app` in Terminal.

Or build from source:

```bash
git clone https://github.com/dacj4n/MacHunt.git
cd MacHunt
```

### CLI only

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

## Requirements

| macOS 15 (Apple Silicon) | ✅ Tested |
| macOS 13–14 | ✅ Expected to work |
| macOS 10.15–12 | ⚠️ Theoretically supported (not tested) |
| Intel Mac (x86_64) | ⚠️ Universal binary included (not tested) |

> **Note**: The app is built as a universal binary (arm64 + x86_64). On macOS <13, login items use AppleScript fallback instead of the modern ServiceManagement API.

- Rust 1.70+
- Node.js 18+ (GUI only)
- npm 9+ (GUI only)

## Quick Start

```bash
# First, build the index (scans your entire disk — takes ~10s for 3M files)
machunt build

# Substring search (case-insensitive)
machunt search "budget"

# Wildcard pattern
machunt search -p "*.rs"

# Fuzzy/typo-tolerant
machunt search -F "redme"

# Case-sensitive
machunt search -c "Makefile"

# JSON output for scripting
machunt search --json "invoice" | jq .

# Start live watcher + interactive search
machunt watch
```

## CLI Reference

```
machunt <COMMAND>
```

### `search`

```bash
machunt search [OPTIONS] <QUERY>
```

| Option | Description |
|--------|-------------|
| `-p, --pattern` | Wildcard/regex mode (e.g. `*.rs`, `test?.txt`) |
| `-F, --fuzzy` | Fuzzy/typo-tolerant search (Levenshtein edit distance) |
| `-c, --case-sensitive` | Case-sensitive matching |
| `-n, --limit <N>` | Max results (default 100) |
| `-P, --path <PATH>` | Path prefix filter |
| `-f, --files` | Files only |
| `-d, --dirs` | Directories only |
| `--json` | JSON output |

Wildcard rules:
- `*` — matches anything except `/` (single directory level)
- `**` — matches anything including `/` (all levels)
- `?` — matches single character except `/`
- `{a,b}` — matches `a` or `b`

### `build`

```bash
machunt build [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-p, --path <PATH>` | Build only this scope |
| `--rebuild` | Clear old index first |
| `--include-dirs <true\|false>` | Include directories (default `true`) |

### `watch`

```bash
machunt watch
```

Starts FSEvents watcher with incremental updates. Resumes from last EventID when available. Drops into an interactive search loop.

### `optimize`

```bash
machunt optimize [--vacuum]
```

Runs WAL checkpoint (always). Optional `--vacuum` reclaims DB file space.

## How It Works

```
┌──────────┐     ┌───────────────┐     ┌──────────┐
│  WalkDir  │ ──→ │  SQLite FTS5  │ ←── │ FSEvents │
│  (build)  │     │  (trigram)    │     │  (watch) │
└──────────┘     └───────┬───────┘     └──────────┘
                         │
                    ┌────▼────┐
                    │  Search  │
                    │ <5ms     │
                    └─────────┘
```

- **Build**: `WalkDir` traverses the filesystem, inserting `(name_lower, path)` into SQLite FTS5 with the trigram tokenizer. Handled in parallel via crossbeam channels.
- **Search**: FTS5 trigram MATCH completes in <5ms (CLI). Case-sensitive queries use a GLOB post-filter (SQLite LIKE is ASCII-case-insensitive by default). Short queries (<3 chars) fall back to LIKE. Fuzzy mode uses Levenshtein distance over LIKE candidates.
- **Watch**: Raw FSEvents FFI (CoreServices) streams file creation, modification, deletion, and rename events. Inserts/updates/deletes from the DB incrementally. Resumes from the last persisted EventID across restarts.

## GUI

The native macOS GUI is built with Tauri 2 and React. It communicates with the same Rust core engine used by the CLI — no HTTP server, no IPC overhead beyond Tauri's native bridge.

### Main Window

- Full-disk search with real-time results
- Navigation tabs: Search / Pinned / Settings (`Cmd+1/2/3`)
- Regex toggle + case-sensitive toggle
- Path filter with suggestion dropdown and Finder picker
- Category tabs: All / Files / Folders / Documents / Images / Media / Code / Archives
- Sortable columns: name, path, type, size, modified
- Draggable column splitters with persisted widths
- Single/multi selection (`Shift` range, `Cmd` additive)
- Keyboard navigation (`↑` `↓`)
- Space-triggered Quick Look (multi-selection supported)
- Double-click to open
- Inline pin button on each result row (hover to reveal)

### Right-Click Menu

Open, Open With... (Finder / QSpace Pro / Terminal / WezTerm), copy name/path, copy as file objects, copy all results, move to Trash, Pin to Favorites.

### Pinned / Favorites

Star any search result to pin it. Pinned items persist in localStorage and survive restarts — no DB mix. The dedicated Pinned tab shows all bookmarked items with full sort, resize, Quick Look, and Cmd+A support. Unpin via the same star button or context menu. Star ⭑ appears at the end of every row (visible on hover). Gold filled = pinned, outline = not.

### Settings Page

- **Theme**: system / light / dark
- **Language**: 中文 / English
- **Shortcut**: global hotkey to show/hide window (default `Cmd+Shift+D`)
- **Startup**: launch at login, silent start, show/hide Dock icon
- **Index Maintenance**: auto `VACUUM` after rebuild on/off
- **Excluded Directories**: exact paths and regex/wildcard patterns
- **Watch Roots**: configure specific subtrees for FSEvents monitoring

## Features

| Category | Capability |
|----------|------------|
| Search modes | Substring, wildcard/regex, fuzzy (Levenshtein) |
| Case sensitivity | Toggleable in both CLI and GUI |
| Path filter | Prefix, suggestion dropdown, Finder picker |
| Live updates | FSEvents watcher, persists EventID across restarts |
| File types | 8 category tabs via extension classification |
| Pinned items | Star button, persistent favorites page, localStorage |
| Preview | Native Quick Look (space bar, multi-file) |
| Export | Copy as file objects, JSON output (CLI) |
| Design | Neomorphic 3D design system, light/dark/system theme |
| i18n | 中文 / English |
| Startup | Launch at login, silent mode, Dock toggle |
| Performance | EventID staleness detection, lazy dead-path cleanup |
| Privacy | 100% local, no network calls |

## Comparison

| | MacHunt | Spotlight | Raycast | uTools |
|---|---|---|---|---|
| **Full disk scan** | Yes (~10s / 3M files) | Yes (`mdfind`) | Plugin-based | Plugin-based |
| **Search latency** | <5ms (CLI, FTS5 trigram) | 50–200ms+ | Varies | Varies |
| **Index format** | SQLite FTS5 (open) | Proprietary | Proprietary | N/A |
| **CLI** | Yes | Yes (`mdfind`) | No | No |
| **Fuzzy search** | Yes (Levenshtein) | Partial | No | No |
| **Incremental update** | FSEvents | FSEvents | Varies | N/A |
| **Open source** | Yes | No | No | Partially |

## Development

### Tech Stack

- **Core**: Rust
- **CLI**: Clap
- **GUI frontend**: React 18 + TypeScript + Vite
- **GUI container**: Tauri 2
- **Global shortcut**: `tauri-plugin-global-shortcut`
- **Storage**: SQLite FTS5 (`rusqlite`, WAL mode, trigram tokenizer)
- **Scanner**: WalkDir + Crossbeam channels
- **Watcher**: macOS FSEvents (CoreServices FFI)
- **Design**: Neomorphic design system with CSS custom properties, inline theme detection for no-flash startup

### Build Commands

| Command | What it does |
|---------|--------------|
| `npm run build` | Build frontend only (TS + Vite → `dist/`) |
| `npm run tauri build` | Full build: frontend + Rust → `.app` / `.dmg` |
| `npm run tauri dev` | Dev mode with hot reload |
| `cargo build --release` | CLI binary only |

### `npm run build` vs `npm run tauri build`

- `npm run build` only builds frontend assets. It does **not** compile Rust, does **not** produce a `.app` or `.dmg`.
- `npm run tauri build` runs `beforeBuildCommand` (which is `npm run build`), then compiles the Rust backend, and produces installable artifacts.

## Project Structure

```
mac_find/
├── src/                    # Core engine (shared by CLI and GUI)
│   ├── main.rs             # CLI entry point (clap)
│   ├── lib.rs              # Library root, re-exports Engine
│   ├── engine.rs           # Engine: build/search/watch orchestration
│   ├── db.rs               # SQLite FTS5: schema, insert, search, fuzzy
│   ├── builder.rs          # WalkDir filesystem scanner
│   ├── watcher.rs          # FSEvents FFI watcher
│   ├── search.rs           # Wildcard-to-regex conversion
│   ├── filters.rs          # Exclude rules (exact + regex/wildcard)
│   └── utils.rs            # Path normalization, skip logic, logger
├── src-tauri/              # Tauri GUI backend
│   ├── src/lib.rs          # Tauri commands, window lifecycle, settings
│   ├── tauri.conf.json     # Tauri configuration
│   ├── Info.plist          # macOS bundle metadata
│   ├── build.rs            # Build script (compiles ObjC bridge)
│   └── macos/
│       └── quicklook_bridge.m  # ObjC bridge: Quick Look, clipboard, Dock
├── src/                    # React frontend (neomorphic design system)
│   ├── App.tsx             # Main app component (~3300 lines, all views)
│   ├── App.css             # Styles (CSS variables, neomorphic theme)
│   └── main.tsx            # Entry point
├── index.html              # HTML shell, inline theme detection script
├── screenshots/            # Screenshots for README
├── scripts/
│   ├── set_version.sh      # Bump version across all config files
│   └── package_release.sh  # Package .app/.dmg for distribution
├── Cargo.toml              # Rust crate manifest
└── package.json            # Frontend dependencies
```

## Runtime Data

| Path | Content |
|------|---------|
| `~/Library/Caches/MacHunt/index.db` | FTS5 search index |
| `~/Library/Application Support/MacHunt/settings.json` | GUI settings |
| `~/Library/Caches/MacHunt/logs/` | Debug logs |

## Why the Index Can Be Large

- Millions of files are common on macOS
- Directory entries are indexed by default
- Long paths dominate storage
- `index.db-wal` can grow temporarily during writes

Maintenance:

```bash
machunt optimize --vacuum
```

## License

MIT
