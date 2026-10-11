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
| File handling | Open with default OS application, create folder, select, copy/paste, move to OS trash with confirmation |
| Live updates | `notify` watches the current directory (non-recursive) |
| Background work | A single background copy operation at a time; no progress or cancellation yet |
| Tests | Isolated filesystem unit/regression tests; Windows CI |

**Not implemented:** tabs, grid thumbnails, drag-and-drop, cut/move, rename, indexed search, multi-selection, undo, copy progress/cancellation and robust recovery after partial transfers.

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

**For now, work on noncritical test data and keep independent backups.**

## Development and quality gates

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

The [Windows workflow](.github/workflows/ci.yml) is the source of truth for automated results; a green check does **not** prove runtime correctness or file operation safety. Manual Windows UI and filesystem tests remain necessary.

## Roadmap

1. **Stability:** passing CI, manual Windows smoke tests, reliable error presentation, basic accessibility.
2. **File operations:** progress and cancellation, reliable staging/recovery, rename/move, multi-selection and undo.
3. **Scale:** async directory scanning, list virtualization, bounded thumbnail cache, indexed search.
4. **Experience:** tabs, previews, keyboard navigation, configurable appearance and polish.

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
