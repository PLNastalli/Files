# Desktop interactions

Files is a Windows-first GPUI Kit application. These controls are implemented in source; feature availability and runtime stability must be confirmed against [Windows CI](https://github.com/PLNastalli/Files/actions) and a manual smoke test.

## File and folder context menu

Right-click a file (in Details or Tiles) to see the **application's** context menu:

- Open; for folders, Open in new tab
- Copy, Cut, Copy path to Windows clipboard
- Rename; Move to Recycle Bin (confirmation required)
- Properties (opens the metadata details pane)
- For folders, Pin / Unpin from Quick Access

This is **not** the native Windows Shell context menu. Installed applications' shell menu items, "Open with", and native Windows property sheets are not present. Copy and Cut are currently *in-app* operations, not Windows Explorer clipboard interoperability.

## Quick Access and This PC

The navigation pane is separated into **Quick Access**, **This PC** drives, and the current folder tree. Default shortcuts use Windows known-folder paths when available. Folder context menus can pin folders; pins persist in the per-user configuration directory under `RustFiles/pinned-folders.txt`. Right-click a pinned shortcut to unpin it.

## Keyboard and navigation

| Shortcut | Action |
| --- | --- |
| Ctrl+A | Select every item in the current folder |
| Escape | Clear selection |
| Ctrl+C / Ctrl+X / Ctrl+V | In-app Copy / Cut / Paste |
| F2 | Rename selected item(s) |
| Delete | Request move to Recycle Bin |
| Alt+Enter | Display metadata details for selected item |
| Shift+Enter | Open selected folder in new tab |
| Ctrl+T / Ctrl+W | New tab / Close tab |
| Alt+Left / Alt+Right / Alt+Up | Back / Forward / Parent |
| F5 | Reload current folder |
| Ctrl+Shift+N | New folder |
| Ctrl+Shift+E | Export CSV listing |

The Ribbon's **Início** page offers New Folder and New Text File. Name conflicts generate a new name rather than overwrite an existing document.

## Still missing compared with the reference

Native Shell context menus, drag-and-drop from Windows Explorer, externally interoperable file clipboard, advanced property sheets, recursive indexed search, full custom shortcut management, and previews for many document formats remain separate engineering tasks. See [FEATURE_PARITY.md](FEATURE_PARITY.md). Test against disposable files while the project remains pre-alpha.
