# Goatpad

Goatpad is a lightweight, native Windows-first editor for Markdown and plain-text notes. It is built with Rust and [egui](https://github.com/emilk/egui), keeps documents local, and restores your workspace between launches.

## Highlights

- Markdown and plain-text documents in persistent tabs
- Live Markdown highlighting for headings, emphasis, strong text, code, links, and lists
- Markdown formatting actions for bold, italic, underline, and bullet lists
- Automatic, atomic document saves in the background
- Workspace restoration, including the active tab, cursor position, scroll position, and window geometry
- Customizable light and dark themes with color, font-family, and font-size controls
- Configurable keyboard shortcuts
- Portable Windows executable with no installer required
- In-app notifications for document, workspace, settings, and theme errors

## Requirements

- Windows is the primary supported platform and release packaging target.
- [Rust](https://www.rust-lang.org/tools/install) with Cargo and a current stable toolchain.
- A Windows C/C++ build environment suitable for compiling Rust desktop dependencies. Visual Studio Build Tools with the Desktop development with C++ workload is a common option.

The application uses egui's bundled font definitions, so it has no runtime dependency on font files.

## Build and run

From the repository root:

```powershell
cargo run
```

For a release build:

```powershell
cargo build --release
```

The release executable is created at:

```text
target\release\goatpad.exe
```

Goatpad is portable. Copy `goatpad.exe` to any folder and run it; no installer or additional runtime files are required. User data is stored separately in the Windows application-data directory, so moving the executable does not move or delete your notes.

## Using Goatpad

### Documents and tabs

- A new installation starts with one persisted `Untitled` Markdown tab.
- Use the `+` button or `Ctrl+T` to create a tab.
- Click a tab to switch documents. The active document name is shown in the window title.
- Use the `×` button or `Ctrl+Shift+W` to delete a tab. Deleting a tab permanently removes its local content file; the final remaining tab cannot be deleted.
- Use the `MD` and `TXT` controls to change the active document type. The content is retained while the underlying file changes between `.md` and `.txt`.
- The footer shows the current line, column, and character count.

Markdown formatting shortcuts only modify Markdown documents. When no text is selected, wrapping actions place the cursor inside the new markers; when text is selected, the selection is wrapped.

### Default keyboard shortcuts

| Action       | Shortcut         |
| ------------ | ---------------- |
| Bold         | `Ctrl+B`         |
| Italic       | `Ctrl+I`         |
| Underline    | `Ctrl+U`         |
| Bullet list  | `Ctrl+Shift+8`   |
| New tab      | `Ctrl+T`         |
| Delete tab   | `Ctrl+Shift+W`   |
| Next tab     | `Ctrl+Tab`       |
| Previous tab | `Ctrl+Shift+Tab` |
| Settings     | `Ctrl+,`         |

Shortcuts can be changed in **Settings**. Click a shortcut, then press the replacement key combination. Invalid saved bindings are ignored and missing actions are restored to their defaults.

### Themes and settings

Open **Settings** with the gear button or `Ctrl+,`.

- Choose the built-in `default-dark` or `default-light` theme.
- Adjust primary, secondary, and background colors.
- Choose the `Sans` or `Monospace` font family.
- Set the font size from 12 to 24.
- Save the current design as a custom theme.
- Rebind any application shortcut.

The selected theme and shortcut changes are saved automatically. Color, font, and size edits are applied immediately as a draft; save them with **Save new theme** if you want to keep them after restarting. These values are stored in the local settings and theme files described below.

## Saving and recovery

Goatpad saves document changes automatically:

- Normal edits are saved after 400 ms without another edit.
- A dirty document is saved no later than 2 seconds after the first edit, even during continuous typing.
- Switching tabs, changing document type, creating or deleting a tab, and exiting the application flush pending changes immediately.
- Writes are performed through a temporary file and rename, reducing the chance of leaving a partially written document behind.

The workspace and session are also saved automatically. On the next launch Goatpad restores the tab list, active tab, cursor and scroll state, and window position and size. If a document file cannot be read, Goatpad keeps the workspace entry, opens that document with empty content, and displays a warning without overwriting the unreadable file.

## Local data layout

On Windows, Goatpad uses the local application-data directory, normally:

```text
%LOCALAPPDATA%\Goatpad
```

The exact base path is provided by the operating system through the `directories` crate. Goatpad creates the following layout:

```text
Goatpad\
├── documents\
│   ├── <document-uuid>.md
│   └── <document-uuid>.txt
├── themes\
│   ├── default-dark.json
│   ├── default-light.json
│   └── <custom-theme>.json
├── settings.json
├── session.json
└── workspace.json
```

- `documents` contains the actual Markdown and text content.
- `workspace.json` stores the tab index, titles, document IDs, and document types.
- `session.json` stores the active tab, per-tab cursor and scroll state, and window geometry.
- `settings.json` stores the selected theme and keyboard shortcuts.
- `themes` stores built-in presets and custom theme definitions.

These are ordinary local files and are not synchronized or backed up by Goatpad. Back up the `Goatpad` directory if you need to preserve the application workspace.

## Development

Useful Cargo commands are:

```powershell
cargo check
cargo test
cargo fmt -- --check
```

To format the source code:

```powershell
cargo fmt
```

The Windows icon is compiled into the executable by `build.rs` from `assets\goatpad.ico`. To regenerate that icon using the included PowerShell script:

```powershell
.\tools\generate_icon.ps1
```

## Project layout

```text
src\
├── main.rs          Application UI, editor behavior, and lifecycle
├── document.rs      Document types and Markdown/text metadata
├── highlighting.rs  Live Markdown and plain-text layout
├── hotkeys.rs       Actions, shortcut parsing, and defaults
├── paths.rs         Application-data paths
├── persistence.rs   Atomic writes and background save worker
├── session.rs       Window, tab, cursor, and scroll restoration
├── settings.rs      Theme and shortcut settings persistence
├── theme.rs         Theme definitions and egui styling
└── workspace.rs     Tab index and document management
```

## Current limitations

- There is no import/export or file-picker workflow yet; documents are managed inside Goatpad's local workspace.
- Release distribution is currently a portable `.exe`, not an MSI or other installer package.
- Windows is the primary supported and packaged platform.
