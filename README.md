# mac_find

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
git clone https://github.com/dacj4n/mac_find.git
cd mac_find
cargo build --release
```

## Usage

### Basic Search

```bash
# Search for files/folders containing "test"
mac_find "test"

# Search only files
mac_find --file "test"

# Search only folders
mac_find --folder "test"
```

### Regex Search

```bash
# Search all pdf files
mac_find --regex "*.pdf"

# Search pdf and docx files
mac_find --regex "*.{pdf,docx}"

# Search files with specific pattern
mac_find --regex "*.mp{3,4}"
```

### Path Filtering

```bash
# Search in specific directory
mac_find --path "/Volumes/Tools" "test"
```

### Index Management

```bash
# Build file index
mac_find build

# Watch file changes in real-time
mac_find watch
```

## Wildcard Rules

- `*` - Match any characters (excluding `/`)
- `**` - Match any characters (including `/`)
- `?` - Match single character (excluding `/`)
- `{a,b}` - Match `a` or `b`

## Examples

```bash
# Search for all video files
mac_find --regex "*.{mp4,mov,avi}"

# Search for large files
mac_find --regex ".*\.pdf"

# Search for test files in specific path
mac_find --path "/Volumes/工作" "测试"

# Monitor file changes
mac_find watch
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
