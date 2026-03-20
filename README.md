# MacHunt

A macOS local file search tool with both CLI and desktop GUI.

[中文文档](README_zh.md)

## Versions

- CLI / Core Engine: `v0.2.0`
- GUI (Tauri + React): `v1.0.0`

## Project Overview

MacHunt provides:

- A Rust-based indexing and search engine.
- A CLI workflow for scripting and terminal usage.
- A native desktop GUI built with Tauri + React.

## Components

- `CLI`: Build index, search, and run file watcher from terminal.
- `Core Engine`: Shared Rust search/index/watch logic.
- `GUI`: Native desktop app that calls local Tauri commands directly.

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

## Quick Start

### 1) Build CLI

```bash
cargo build --release
```

### 2) Start GUI (Development)

```bash
npm install
npm run tauri dev
```

`tauri dev` loads local static assets from `dist` (built by `npm run build`).
No extra HTTP backend service is required.

### 3) Build GUI (Production)

```bash
npm run build
npm run tauri build
```

## CLI Usage

### Build Index

```bash
# Full scan
machunt build

# Build for a specific path
machunt build --path "/Volumes/Tools"

# Force rebuild
machunt build --rebuild
```

### Search

```bash
# Search files and folders
machunt "test"

# Search files only
machunt --file "test"

# Search folders only
machunt --folder "test"

# Search with path prefix
machunt --path "/Volumes/Tools" "test"
```

### Pattern Search

```bash
machunt --regex "*.pdf"
machunt --regex "*.{pdf,docx}"
machunt --regex "*.mp{3,4}"
```

### Watch Mode

```bash
machunt watch
```

## GUI Notes

- GUI uses the same Rust core engine as CLI.
- Search is local and direct via Tauri invoke.
- No REST/HTTP server is required for search requests.
- Index/watcher behavior is shared with CLI.

## Permissions

For full-disk indexing/monitoring on macOS, grant **Full Disk Access**:

1. Open **System Settings → Privacy & Security → Full Disk Access**.
2. Add your terminal app for CLI usage.
3. Add the generated MacHunt app for GUI usage.
4. Restart related apps after granting permission.

Without this permission, coverage may be limited.

## Wildcard Rules

- `*`: any chars except `/`
- `**`: any chars including `/`
- `?`: single char except `/`
- `{a,b}`: `a` or `b`

## Performance (Reference)

- Index build: ~10-15s for ~2M files
- Query latency: typically <50ms
- Memory usage: ~200-300MB for ~2M files

## Architecture

- Index storage: SQLite (WAL mode)
- File watch: macOS FSEvents
- Concurrent map: DashMap
- Parallel scan: Crossbeam + WalkDir
- Desktop stack: Tauri + React

## License

MIT
