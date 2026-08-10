<p align="center"><img src="src-tauri/icons/icon.png" width="220" alt="MacHunt icon" /></p>

<h1 align="center">MacHunt</h1>

<p align="center">
  A private, fully local file search app for macOS.<br>
  The new beta is written entirely in Swift with native Apple frameworks.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Swift-6-F05138?logo=swift&logoColor=white" alt="Swift 6" />
  <img src="https://img.shields.io/badge/UI-SwiftUI%20%2B%20AppKit-147EFB?logo=apple&logoColor=white" alt="SwiftUI and AppKit" />
  <img src="https://img.shields.io/badge/Index-SQLite%20FTS5-003B57?logo=sqlite&logoColor=white" alt="SQLite FTS5" />
  <img src="https://img.shields.io/badge/macOS-14%2B-black?logo=apple" alt="macOS 14 or later" />
</p>

<p align="center"><a href="README_zh.md">中文文档</a></p>

> [!IMPORTANT]
> The native Swift version is currently a beta. Its source is in [`swift_beta`](swift_beta/). The original Tauri/React/Rust implementation is preserved in the repository root and has not been removed.

## What is MacHunt?

MacHunt builds a local SQLite FTS5 index of file and folder metadata, then provides fast filename search through a native macOS app and CLI. Search data stays on the Mac: there is no account, HTTP backend, analytics service, or cloud search API.

The Swift beta uses native SwiftUI and AppKit controls together with Quick Look, Finder integration, FSEvents, ServiceManagement, and SQLite. It is a separate implementation and does not link the legacy Rust core.

## Screenshots

| Search | Pinned |
|:--:|:--:|
| [![Search](screenshots/swift/search.png)](screenshots/swift/search.png) | [![Pinned](screenshots/swift/pinned.png)](screenshots/swift/pinned.png) |
| Native result table, filters, metadata, and file actions | Persistent favorites with the same Finder-style controls |

| Quick Look | Settings |
|:--:|:--:|
| [![Quick Look](screenshots/swift/quick-look.png)](screenshots/swift/quick-look.png) | [![Settings](screenshots/swift/settings.png)](screenshots/swift/settings.png) |
| Press Space to use the system Quick Look panel | Index, categories, exclusions, appearance, and integration settings |

## Install the beta

1. Download the latest Swift beta `.dmg` from [GitHub Releases](https://github.com/Jingyuan-Zheng/MacHunt_SwiftUI/releases).
2. Open the disk image.
3. Drag `MacHunt.app` to Applications.

The beta is ad-hoc signed and is not notarized. If Gatekeeper blocks the first launch, right-click the app in Finder and choose **Open**. If macOS still reports a damaged app, remove the quarantine attribute from the copy you trust:

```bash
xattr -cr /Applications/MacHunt.app
```

The published beta currently targets Apple silicon and requires macOS 14 or later.

## First use

MacHunt can build the first index from the app. For a full-disk CLI build with visible progress, quit MacHunt and run:

```bash
"/Applications/MacHunt.app/Contents/Helpers/machunt" build --path /
```

The build reports four phases:

```text
Scanning files
Building search index
Optimizing database
Finishing index
```

Open MacHunt after the command prints the final indexed-item count. Later file changes are maintained with FSEvents.

## Native Swift beta features

- Native SwiftUI windows and AppKit result table using the standard macOS appearance
- Substring, wildcard/regular-expression, fuzzy, and case-sensitive search
- Path, file category, application, date, and size filters
- Editable built-in file categories and unlimited custom categories
- SF Symbol and emoji icons for custom categories
- Finder-style selectable columns, sorting, multi-selection, and keyboard navigation
- Name, path, type, size, modified date, added date, Finder tags, and iCloud status
- Open, Open With, Reveal in Finder, Copy, Move to Trash, Pin, and context-menu actions
- System Quick Look with Space, including multi-file preview
- Pinned items that persist across launches
- FSEvents incremental index maintenance and external-volume monitoring
- Configurable index roots, excluded paths/patterns, hidden files, and result limits
- Native menu commands, global shortcut, launch at login, and Dock visibility controls
- English and Simplified Chinese localization
- Shared Swift CLI and GUI index

## Privacy and iCloud files

MacHunt indexes names, paths, dates, sizes, Finder tags, and cloud status. It does not index file contents and does not intentionally download online-only iCloud files during scanning. Opening or using Quick Look on an online-only result may ask macOS to download that file.

Runtime data is stored locally:

```text
~/Library/Caches/MacHuntSwift/index.db
```

Rebuilds are atomic: MacHunt creates `index.db.new`, keeps the previous completed database available for search, and replaces it only after the new index is complete.

## CLI

The CLI is embedded in the app at `MacHunt.app/Contents/Helpers/machunt`.

```bash
machunt build --path /
machunt status
machunt search invoice
machunt search --fuzzy --limit 50 "project report"
machunt search --pattern "*.swift"
machunt search --path ~/Documents --json budget
machunt optimize
```

Search options:

| Option | Description |
|---|---|
| `-p`, `--pattern` | Wildcard or regular-expression mode |
| `-F`, `--fuzzy` | Ordered fuzzy subsequence matching |
| `-c`, `--case-sensitive` | Preserve case |
| `-P`, `--path <path>` | Limit results to a path prefix |
| `-f`, `--files` | Files only |
| `-d`, `--dirs` | Folders only |
| `-n`, `--limit <count>` | Maximum result count |
| `--json` | JSON output |

## Build the Swift beta

```bash
git clone https://github.com/Jingyuan-Zheng/MacHunt_SwiftUI.git
cd MacHunt_SwiftUI/swift_beta
swift test
scripts/build-app.sh release
open .build/MacHunt.app
```

Requirements: Xcode with Swift 6.2 or later and macOS 14 or later. See [`swift_beta/README.md`](swift_beta/README.md) for architecture and development details.

## Repository layout

```text
MacHunt_SwiftUI/
├── swift_beta/        # Native SwiftUI/AppKit app, core, CLI, tests, packaging
├── screenshots/swift/ # Current native beta screenshots
├── src-tauri/         # Legacy Tauri application container
├── src/               # Legacy Rust core and React frontend sources
├── Cargo.toml         # Legacy Rust package
└── package.json       # Legacy React/Tauri tooling
```

## Legacy Tauri version

The original Tauri version remains buildable and is retained unchanged for comparison:

```bash
npm install
npm run tauri dev
```

Its Rust CLI can be built with `cargo build --release`. New native development is taking place in `swift_beta`.
