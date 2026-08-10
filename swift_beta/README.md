# MacHunt Swift Beta

`swift_beta` contains the fully native macOS rewrite of MacHunt. It is implemented in Swift 6 with SwiftUI, AppKit, SQLite FTS5, FSEvents, Quick Look, Finder integration, and ServiceManagement. It does not depend on the legacy Tauri, React, Node.js, or Rust implementation.

The legacy Tauri/Rust application remains in the repository root for reference and compatibility.

## Requirements

- macOS 14 or later
- Xcode with Swift 6.2 or later
- Apple silicon for the published beta build

## Build

Open `Package.swift` in Xcode and run the `MacHunt` executable, or build from Terminal:

```bash
cd swift_beta
swift test
scripts/build-app.sh release
open .build/MacHunt.app
```

Use `scripts/build-app.sh debug` for a debug app bundle. The script packages the GUI and CLI into `.build/MacHunt.app` and applies an ad-hoc signature for local use.

The packaged CLI is located at:

```text
.build/MacHunt.app/Contents/Helpers/machunt
```

Examples:

```bash
.build/MacHunt.app/Contents/Helpers/machunt build --path /
.build/MacHunt.app/Contents/Helpers/machunt status
.build/MacHunt.app/Contents/Helpers/machunt search --limit 100 invoice
```

## Architecture

```text
MacHuntApp (SwiftUI + AppKit)
        │
        ├── native NSTableView results and context menus
        ├── Quick Look, Finder, Trash, clipboard, login item
        └── FSEvents live monitoring
        │
MacHuntCore
        │
        ├── parallel metadata scanner with a bounded queue
        ├── normalized SQLite storage
        ├── contentless FTS5 trigram filename index
        └── substring, pattern, fuzzy, path, type, date and size filters
        │
MacHuntCLI
```

Directory paths are stored once in `directories`; file rows reference them by integer ID. Finder tags use sparse relation tables, and cloud state uses a compact integer enum. Rebuilds use `index.db.new`, keeping the previous completed index searchable until the new database has been scanned, indexed, optimized, validated, and atomically installed.

## Runtime data

```text
~/Library/Caches/MacHuntSwift/index.db
```

The index is a rebuildable local cache. Search terms, file metadata, pinned items, and settings remain on the Mac. Scanning reads metadata from iCloud placeholders but does not intentionally download file contents; opening or previewing an online-only file may cause macOS to download it.

## Implemented features

- Native search, pinned-items, and Settings windows
- Substring, wildcard/regular-expression, fuzzy, and case-sensitive search
- Path, file category, application, date, and size filtering
- Editable built-in categories and unlimited custom categories with SF Symbol or emoji icons
- User-selectable Finder-style result columns, sorting, multi-selection, and keyboard navigation
- File size, modified date, added date, Finder tags, and iCloud status columns
- Open, Open With, Reveal in Finder, Copy, Move to Trash, Pin, and native Quick Look
- FSEvents incremental maintenance with saved cursors and dropped-event recovery
- Configurable roots, exclusions, external volumes, hidden-file handling, and result limits
- English and Simplified Chinese localization
- Native menu commands, global show/hide shortcut, Dock visibility, and launch at login
- Shared CLI using the same Swift core and index

## Tests

```bash
swift test
```

The beta is ad-hoc signed and is not notarized. Production distribution signing, notarization, automatic updates, and Intel builds remain release engineering work.
