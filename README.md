# Files

Native file manager for Windows written in Rust and rendered with [GPUI](https://github.com/zed-industries/zed). Inspired by the layout and workflow of Windows File Explorer, without copying Microsoft's code or assets.

> **Status: early MVP.** This repository is not yet a replacement for Windows File Explorer. The UI and core filesystem operations still need compilation and hands-on Windows validation.

## Goals

- Fast, GPU-rendered desktop interface in Rust (no Electron, HTML or webview)
- Sidebar shortcuts, folder navigation, breadcrumbs, history, sorting
- Safe file operations with no accidental overwrites
- Keyboard shortcuts, accessibility, tabs, thumbnail previews and robust background transfers in future iterations

## Stack

| Area | Technology |
| --- | --- |
| Interface | GPUI (Zed), native Rust |
| Filesystem | `std::fs`, `walkdir` |
| Change notifications | `notify` |
| Asynchronous work | GPUI executor; Tokio planned for background transfer service |
| Tests | Rust unit tests; GitHub Actions on Windows |

## Run

Requires the latest stable Rust toolchain and Windows 11/10.

```powershell
git clone https://github.com/PLNastalli/Files.git
cd Files
cargo run --release
```

The pinned GPUI Git revision means the first build must download a substantial dependency tree. `cargo test --lib` runs the filesystem tests.

## First MVP controls

- Click a sidebar location to open it.
- Double-click folders to enter them; double-click a file to open with its default application.
- Back, Forward, Up, Refresh and sorting in the top bars.
- Select a file or directory, use **Copy**, then **Paste** into another folder. Name conflicts create a new name rather than overwrite.
- **New folder** creates a uniquely named directory.
- **Move to Trash** requires confirmation and uses the OS trash/recycle bin.
- File watchers refresh the current directory when it changes.

## Safety and limitations

- Symbolic links are shown but never followed during recursive copy.
- Large copies currently run synchronously and can briefly block the UI. Nonblocking transfers with progress and cancellation are next.
- No cut/move, rename, search indexing, thumbnails, or tabs yet.
- Cross-platform UI is a design goal; initial CI targets Windows.
- Test the application on noncritical files before relying on it.

## License

MIT.
