# Contributing to Files

Thanks for helping improve this experimental Rust file manager.

## Development setup

Use Windows with the Rust stable MSVC toolchain. From the repository root:

```powershell
cargo fmt --all
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

CI runs the supported checks on Windows. Please include the relevant results in your pull request.

## Code expectations

- Prefer explicit, idiomatic Rust. Avoid speculative abstractions, needless clones and unchecked panics.
- Keep filesystem behavior testable independently of GPUI Kit. UI rendering belongs in `src/ui.rs`; application coordination in `src/main.rs`; low-level filesystem behavior in `src/filesystem.rs`.
- Handle `Result` failures and race conditions deliberately; do not suppress errors just to pass the build.
- Avoid synchronous expensive file work on the GPUI foreground thread.
- Add regression tests for copying, name collisions, permissions, symbolic links, and cancellation semantics whenever affected.
- Do not claim a performance improvement without a representative benchmark and its setup.
- Update the README when adding features, changing behavior, dependencies, keyboard shortcuts, or limitations.

## Pull request checklist

- [ ] The change is focused and explains why it is needed.
- [ ] Formatting, compilation, linting and tests pass on the targeted Windows toolchain.
- [ ] New or changed filesystem behavior has tests, including failure paths.
- [ ] File operations were tested with disposable directories and do not overwrite existing data.
- [ ] Screenshots accompany visual/UI changes when useful.
- [ ] No credentials, proprietary icons, or third-party copyrighted code are included.

## Reporting bugs

Please include the Windows version, Rust version (`rustc -V`), reproduction steps, expected versus observed behavior, and the full build error or crash log. Remove personal file paths and sensitive information from logs.
