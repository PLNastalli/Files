# Changelog

All changes below refer to the experimental development branch (`main`). There are no stable releases yet. See [feature parity](docs/FEATURE_PARITY.md) for incomplete reference features and [GitHub Actions](https://github.com/PLNastalli/Files/actions) for build verification.

## Unreleased — October 2026

### Added
- Windows 11-inspired GPUI Kit interface, Explorer-like navigation, folder tabs and Details/Tiles layout.
- GPUI Kit sidebar, toolbar, icons, search input and optional metadata details panel.
- Copy/paste with conflict-safe filenames; cut/paste move supported on the same filesystem.
- Single-item rename with Windows name validation.
- Ctrl-click multi-selection and numbered-prefix batch rename with preflight checks and best-effort rollback.
- Current-folder CSV export with UTF-8 BOM, non-overwriting names, quoted cells and spreadsheet formula hardening.
- Row-number toggle in the Details table.
- Dedicated filesystem mutation module and associated regression tests.
- Strict Windows CI commands and a reference-feature parity tracker.

### Known limitations
- Compilation checks do not substitute for hands-on testing under Windows with disposable files.
- Copy/move/rename/trash do not yet support a unified safe multi-item transaction model.
- No progress/cancel/recovery for copies, no recursive search, no native previews, and no Shell drag-and-drop.
- Full reference parity, formula batch rename, custom columns, context menu integration and theme packs are still pending.
