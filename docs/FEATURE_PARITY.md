# Feature parity with omeryanar/FileExplorer

The reference project ([omeryanar/FileExplorer](https://github.com/omeryanar/FileExplorer)) is a substantial C#/WPF/DevExpress Windows application. Files is an independent Rust + GPUI Kit implementation. **Do not describe visual similarity as feature parity.**

Last manually reviewed: October 2026. Status describes source implementation, not verified end-user reliability.

| Reference feature | Files status | Gap / required work |
| --- | --- | --- |
| Tabs | Partial | Multiple tabs and switching; drag to detach, per-tab history and middle-click open are missing |
| Combined navigation / tree content | Not implemented | Expandable hierarchy, virtualized subtrees, keyboard navigation |
| Batch rename with formulas | Not implemented | Preview, validated templates, batch atomicity/recovery |
| Standard Windows context menu | Not implemented | Shell COM integration and application-custom menu configuration |
| Custom context menu expressions | Not implemented | Safely modeled actions and arguments, opt-in execution |
| Basic columns (name, type, size, dates) | Partial | Name/type/size/modified available; created date and column customization missing |
| Media metadata columns | Not implemented | Image, audio, video property extractors and optional codecs |
| Custom/calculated columns | Not implemented | Safe expression parser, schema and saved definitions |
| Preview pane | Partial | Metadata details only; no file-content previews yet |
| Advanced filtering | Partial | Current-folder name substring only; no filter builder |
| Conditional formatting | Not implemented | Rules, schema, editing and persisted styling |
| Save/load layout | Not implemented | Versioned config file with migration and defaults |
| Tree sorting | Not implemented | Expandable tree with sort model |
| Windows drag-and-drop | Not implemented | OLE IDataObject, external drags, move/copy rules |
| Row numbers | Not implemented | Index column and persisted toggle |
| Print and export | Partial | CSV export only; print and PDF/Excel not available |
| Themes | Partial | Windows 11-inspired light palette; no other themes or system-follow |
| Create folder | Implemented, test required | Name conflicts handled |
| Rename (single) | Implemented, test required | Inline edit + conflict validation; still lacks full Shell semantics |
| Copy (single) | Implemented, test required | Background copy, no progress/cancel or atomic recovery |
| Cut/move (single) | Implemented, test required | Same-volume rename only; cross-volume workflow missing |
| Trash | Implemented, test required | One selected item, confirmation and OS recycle bin |

## Implementation order

1. **Correctness gate:** Windows CI must pass `cargo fmt`, `cargo check`, Clippy without warnings, core tests, release build. Manual Windows smoke tests remain mandatory.
2. **Reliable operations:** multi-select, file name input validation, batch rename with preview and rollback, move across volumes with careful recovery, transfer progress and cancellation.
3. **Navigation at scale:** independent tab state, virtualized rows, async directory scan, tree expansion, drop targets and full keyboard controls.
4. **Extensions and filters:** custom columns, property extractors, formula/parser engine, advanced filter builder and conditional formatting.
5. **OS integration:** safe Windows context menus, shell drag-and-drop, thumbnail extraction, document/image/audio/video/PDF preview adapters.
6. **Persistence and exports:** versioned UI settings, layout profiles, saved filters, print dialogs, PDF/Excel outputs.
7. **Theming and performance:** system-follow light/dark, optional themes, accessibility, profiling on large trees and Windows compatibility tests.

Each feature must come with unit/integration coverage and updated screenshots/README before it can be declared complete.

## Safety priorities

Windows Shell integration can execute handlers supplied by third-party installed software. Preview providers may parse untrusted or malformed files. Formula/context-action evaluation must never silently run shell commands. Default behavior should prefer explicit user action, bounded work, and non-overwriting operations.

The application's file operations are **not transactional yet**. In particular, a failed recursive copy may leave partial output; preserve backups and use disposable data for now.
