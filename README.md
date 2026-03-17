# MacHunt

macOS Global File Search Tool - Similar to Windows' Everything

[中文文档](README_zh.md)

## Features

- ⚡ **High Performance**: Search entire disk in seconds
- 🔍 **Real-time Monitoring**: Monitor file changes in real-time
- 📂 **Flexible Search**: Support substring, regex, folder/file filtering
- 🔄 **Incremental Indexing**: Fast rebuild with event ID persistence
- 🗄️ **Persistent Storage**: SQLite-based index storage

## Requirements

- macOS 10.15+
- Rust 1.70+

## Installation

```bash
git clone https://github.com/dacj4n/MacHunt.git
cd MacHunt
cargo build --release
```

## Usage

### First-time Setup

```bash
# Build file index (required before first use)
machunt build          # Full disk scan, ~15-20 seconds

# Or build index for specific path
machunt build --path "/Volumes/Tools"
```

### Search

```bash
# Search for files/folders containing "test"
machunt "test"

# Search only files
machunt --file "test"

# Search only folders
machunt --folder "test"
```

### Regex Search

```bash
# Search all pdf files
machunt --regex "*.pdf"

# Search pdf and docx files
machunt --regex "*.{pdf,docx}"

# Search files with specific pattern
machunt --regex "*.mp{3,4}"
```

### Path Filtering

```bash
# Search in specific directory
machunt --path "/Volumes/Tools" "test"
```

### Real-time Monitoring

```bash
# Start real-time monitoring (keep running in background)
machunt watch

# Then search from anywhere
machunt "test"
```

## Permissions

**Important**: To monitor all directories, you need to grant Full Disk Access to your terminal:

1. Go to **System Settings → Privacy & Security → Full Disk Access**
2. Click the lock icon and enter your password
3. Click **+** and add your terminal app (Terminal.app, iTerm2, etc.)
4. Restart your terminal

Without this permission, monitoring will only work for `/Users` directory.

## Wildcard Rules

- `*` - Match any characters (excluding `/`)
- `**` - Match any characters (including `/`)
- `?` - Match single character (excluding `/`)
- `{a,b}` - Match `a` or `b`

## Examples

```bash
# Search for all video files
machunt --regex "*.{mp4,mov,avi}"

# Search for large files
machunt --regex ".*\.pdf"

# Search for test files in specific path
machunt --path "/Volumes/工作" "测试"

# Monitor file changes
machunt watch
```

## Performance

- Index building: ~10-15 seconds for 2M files
- Search response: <50ms for typical queries
- Memory usage: ~200-300MB for 2M files

## Architecture

- **Index Storage**: SQLite database with WAL mode
- **File Monitoring**: FSEvents API for macOS
- **Concurrent Access**: DashMap for lock-free access
- **Parallel Processing**: Crossbeam for multi-threaded scanning

## License

MIT
