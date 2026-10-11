# Files

**Native Windows file manager built with Rust and GPUI Kit.**

[![Windows CI](https://github.com/PLNastalli/Files/actions/workflows/ci.yml/badge.svg)](https://github.com/PLNastalli/Files/actions/workflows/ci.yml)
[![Rust edition](https://img.shields.io/badge/Rust-2024-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Files is an experimental GPU-rendered desktop file manager that combines an Explorer-inspired workflow with native Rust UI components. The interface uses [GPUI Kit](https://gpui-kit.com/component/) and the filesystem layer uses the Rust standard library. There is no Electron, webview, or embedded browser.

> **Project status: pre-alpha / experimental.** This is neither a drop-in replacement for Windows File Explorer nor a safe choice for important files yet. Check the latest [Windows CI results](https://github.com/PLNastalli/Files/actions) and use disposable test directories while evaluating it.

## Features

| Area | Current implementation |
| --- | --- |
| Native UI | GPUI Kit 0.7.1, themed controls, Lucide icons, sidebar, toolbar and status bar |
| Navigation | Home and common-folder shortcuts, local drives, breadcrumbs, Back, Forward, Up, Refresh |
| Directory listing | Name/size/modified ordering, folders first, optional hidden entries |
| Search | Case-insensitive filter of the **current directory only**; no recursive index |
| Explorer-like UI | Windows 11-inspired light palette, Ribbon, tabs, Details/Tiles, metadata panel, Ctrl-click selection and optional row numbers |
| File handling | Open, create folder or blank text file, rename one item, numbered batch rename, copy and same-volume cut/move, recycle bin |\n| Context menu | GPUI Kit right-click on file rows/tiles with Open, Preview, Copy, Cut, Rename, Trash, Properties (metadata), Copy Path and pin folders |\n| Quick Access | Windows known folders, local drives, user-pinned folders saved under the user's configuration directory |
| Preview pane | Read-only PNG/JPEG/GIF/WebP/BMP and bounded UTF-8 text/code previews (64 KiB); PDF and Office files are not supported |
| Export | Current directory listing to non-overwriting CSV (UTF-8 BOM; spreadsheet formula prefixes sanitized) |
| Live updates | `notify` watches the current directory (non-recursive) |
| Background work | One copy, move or CSV export task at a time; no progress or cancellation yet |
| Tests | Isolated filesystem unit/regression tests; Windows CI |

**Not implemented:** virtualized tree, grid thumbnails, PDF/Office/audio/video previews, drag-and-drop, cross-volume cut/move, formula-based batch rename, indexed search, undo, copy progress/cancellation and robust recovery after partial transfers. Numbered batch rename and Ctrl-click multi-selection are experimental and don't yet extend to every file operation. See the [feature-parity tracker](docs/FEATURE_PARITY.md) for the full comparison with the reference app.

## Getting started

### Requirements

- Windows 10 or 11 (the development and CI target)
- [Rust stable](https://rustup.rs/) and a suitable MSVC build toolchain
- Git

### Build and run

```powershell
git clone https://github.com/PLNastalli/Files.git
cd Files
cargo run --release
```

To update an existing clone:

```powershell
git pull --ff-only
cargo run --release
```

First builds may take several minutes because the GPUI UI toolkit has substantial native dependencies.

### Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| Alt + Left / Right | Back / Forward |
| Alt + Up | Parent directory |
| F5 | Refresh |
| Ctrl + Shift + N | Create a folder |
| Ctrl + C / Ctrl + V | Copy selected item / Paste into current folder |
| Ctrl + X | Cut selected item for same-volume move |
| F2 | Rename one selected item; use a numbered prefix when multiple items are selected |
| Ctrl + Shift + E | Export current folder listing as CSV |
| Ctrl + T / Ctrl + W | Open a new tab / Close active tab |\n| Shift + Enter | Open a selected folder in a new tab |\n| Alt + Enter | Show metadata panel for selected item |\n| Ctrl + A | Select all items in current directory |\n| Escape | Clear file selection |
| Ctrl + Shift + 1 | Switch between Details and Tiles |
| Alt + Shift + P | Toggle metadata details panel |
| Ctrl + Shift + P | Toggle read-only file preview panel |
| Ctrl + Shift + L | Toggle row-number column |
| Ctrl + click | Select or deselect multiple files for numbered batch renaming |\n| Right-click file or folder | Open GPUI Kit context menu with file operations |
| Enter | Open selected item |
| Delete | Ask before sending selected item to Trash |
| Ctrl + H | Toggle hidden files |

Shortcuts and the searchable input are under active development; report any keyboard focus regressions.

## Architecture

The project intentionally remains **one Cargo package**. Logic is separated by responsibility instead of adding unnecessary service layers or traits.

```text
src/
├── main.rs         # Application state, navigation, watcher and task lifecycle
├── ui.rs           # GPUI Kit rendering and input controls
├── filesystem.rs   # Directory reads, sorting, safe-copy helpers and regression tests
├── operations.rs   # Windows-first rename/move/CSV mutation helpers and tests
└── lib.rs          # Filesystem module exposed to the test target
```

| Dependency | Purpose |
| --- | --- |
| `gpui-kit` | GPUI-compatible desktop components and default icon assets |
| `walkdir` | Traversing folders during recursive copy |
| `notify` | Local directory change notifications |
| `trash` | Operating-system recycle bin integration |
| `dirs` | User home directory |
| `open` | Opening files through their associated application |
| `chrono` | Formatting file modification times in the UI |

**Behavioral boundaries:** the UI schedules copying on the GPUI background executor; the `filesystem` module contains the copy algorithm. No GUI dependency is required for filesystem unit tests.

## Safety and limitations

File handling deserves more scrutiny than a typical UI demo:

- The copy helper never intentionally overwrites existing files; file creation uses `create_new` and directory creation uses `create_dir`.
- Symbolic links are rejected during copy, including symbolic links inside a directory tree. The app does not follow them.
- Copying a folder into its own descendants is rejected.
- Destination conflicts result in a copy-suffixed name.
- The copy operation is **not transactional**. Disk-full errors, permission changes, unexpected exits or cancellation can leave *partial files or directories* at the destination. Partial-copy cleanup, atomic staging and resumable transfers are **not implemented**.
- Directory metadata may change between scanning and reading a file, so listing and transfer operations can fail due to races in the underlying filesystem.
- The file watcher monitors only the active folder; it does not build an index.
- Very large directory listings are rendered eagerly, and browsing can still block the UI during metadata scans. Paging/virtualization and asynchronous scans are future work.
- Filesystem paths are not a security boundary: do not treat this build as a sandbox for untrusted paths.

**Right-click menu scope:** This is an application-level GPUI Kit menu, not the native Windows Shell menu. Third-party "Open with", context-menu extensions, ACL/property sheets, and thumbnail providers require separate OS integration. The Properties item currently opens Files' metadata panel. Quick Access pins persist, but special folders are resolved from Windows known-folder paths only.

**For now, work on noncritical test data and keep independent backups.**

## Development and quality gates

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

The [feature-parity tracker](docs/FEATURE_PARITY.md) records which reference FileExplorer features still need engineering. The [Windows workflow](.github/workflows/ci.yml) is the source of truth for automated results; a green check does **not** prove runtime correctness or file operation safety. Manual Windows UI and filesystem tests remain necessary.

## Roadmap

1. **Stability:** passing CI, manual Windows smoke tests, reliable error presentation, basic accessibility.
2. **File operations:** progress/cancellation, reliable staging/recovery, full formula batch rename, multi-item copy/move/trash and undo.
3. **Scale:** async directory scanning, list virtualization, bounded thumbnail cache, indexed search.
4. **Experience:** previews, better per-tab navigation, keyboard navigation, configurable appearance and polish.

## Contributing

Before opening a pull request, run the quality gates above and describe what changed, how you tested it, and any known limitations. Please prefer focused changes that preserve existing behavior. See [CONTRIBUTING.md](CONTRIBUTING.md) for the contributor checklist.

## Design references

We studied established projects for documentation and engineering conventions; Files is independent of their code:

- [Zed](https://github.com/zed-industries/zed) — native Rust editor and GPUI implementation
- [Yazi](https://github.com/sxyazi/yazi) — file manager architecture and task lifecycle
- [ripgrep](https://github.com/BurntSushi/ripgrep) — transparent features, limitations and measured performance claims
- [bat](https://github.com/sharkdp/bat) — clear usage instructions, platform notes and examples
- [Helix](https://github.com/helix-editor/helix) — concise setup and contribution structure

No performance benchmark or reliability guarantees are claimed without supporting measurements.

## License

Distributed under the [MIT License](LICENSE).
