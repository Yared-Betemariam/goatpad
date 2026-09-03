# Goatpad

Goatpad is a lightweight desktop editor for Markdown and plain-text notes. It is built with Rust and egui, designed primarily for Windows, and keeps your notes stored locally.

## Features

- A Notepad-style look throughout: a custom title bar with tabs, a classic File/Edit/View menu bar, and a Markdown formatting toolbar
- Markdown and plain-text editing in persistent tabs, switchable per note (`MD`/`TXT`)
- A Markdown formatting toolbar (shown for Markdown notes) with headings, bulleted/numbered lists, bold, italic, strikethrough, links, tables, and clear formatting
- A searchable Tabs List for reopening closed notes and explicitly deleting unwanted notes
- Automatic note titles derived from the first line, with custom titles via double-click on a tab (renamed in place)
- Import and export of all notes through portable JSON backups
- Live Markdown highlighting and common formatting actions, available from the toolbar, the Edit menu, or keyboard shortcuts
- A Notepad-style status bar with cursor position, character count, document type, zoom control, line-ending, and encoding
- Automatic background saving
- Workspace and window restoration between launches
- Custom light and dark themes
- Configurable keyboard shortcuts
- Local storage with no account or cloud service required

## Requirements

- 64-bit Windows
- A current stable Rust toolchain with Cargo
- A Windows C/C++ build environment, such as Visual Studio Build Tools
- The .NET SDK and WiX Toolset 7 when building an MSI installer

Install WiX once with:

```powershell
dotnet tool install --global wix --version 7.0.0
```

## Run locally

Clone the repository, open a terminal in the project directory, and run:

```powershell
cargo run
```

To create an optimized build:

```powershell
cargo build --release
```

The executable will be available at:

```text
target\release\goatpad.exe
```

The release executable is portable and can be run without an installer.

## Update the application icon

`assets/raw-icon.png` is the single source image for Goatpad's application icon. Replace that file with any PNG you want to use, then run a normal Cargo command such as:

```powershell
cargo build
```

The build automatically centers the image without changing its aspect ratio, regenerates the multi-resolution `assets/icon.ico` and runtime `assets/icon.rgba`, embeds the icon in the Windows executable, and updates the window and title-bar icons. The MSI installer also uses the generated `.ico` file. A square PNG with transparency and a high resolution (at least 256×256) produces the best results.

## Build the MSI installer

Run the packaging script from the project directory:

```powershell
.\build-msi.cmd
```

The script builds the latest release executable, prepares the required WiX extension, and creates:

```text
dist\Goatpad-<version>-x64.msi
```

The version is read from `Cargo.toml`. To build the MSI and immediately open Windows Installer, run:

```powershell
.\build-msi.cmd --install
```

The installer adds Goatpad to the Start menu and desktop, and installs it under `Program Files`. Rebuilding and installing the MSI replaces the existing installation even when the version is unchanged, so local development builds can be reinstalled directly. For identifiable releases, update the package version in `Cargo.toml` before building.

Close Goatpad before upgrading. Uninstalling or upgrading the application does not remove documents or settings from `%LOCALAPPDATA%\Goatpad`.

## Keyboard shortcuts

| Action                    | Shortcut                        |
| ------------------------- | ------------------------------- |
| New tab                   | `Ctrl+T`                        |
| Close tab                 | `Ctrl+Shift+W`                  |
| Next tab                  | `Ctrl+Tab`                      |
| Previous tab              | `Ctrl+Shift+Tab`                |
| Settings                  | `Ctrl+,`                        |
| Bold / Italic / Underline | `Ctrl+B` / `Ctrl+I` / `Ctrl+U`  |
| Strikethrough             | `Ctrl+Shift+X`                  |
| Bulleted / Numbered list  | `Ctrl+Shift+8` / `Ctrl+Shift+7` |
| Insert link               | `Ctrl+K`                        |

Formatting shortcuts only apply to Markdown notes. Shortcuts can be changed from the application settings.

## The Markdown formatting toolbar

Markdown notes show a toolbar above the editor, mirroring Notepad's Markdown view: a heading dropdown (H1-H3), a list dropdown (bulleted/numbered), bold, italic, strikethrough, link, table, and clear-formatting buttons. Plain-text notes hide these tools since Markdown syntax does not apply. The `MD`/`TXT` switch at the right of the toolbar (also available under the `View` menu) changes a note's type at any time.

## Note titles

New notes are named `Untitled` until you type on the first line. Goatpad then uses up to the first 32 characters of that line as the note title. Double-click a tab to rename it in place; press `Enter` to confirm or `Escape` to cancel. Clear the custom title to return to automatic naming.

## Import and export

Open the `File` menu and choose `Export notes…` to save every stored note—including notes that are currently closed—to a portable JSON backup. Choose `Import notes…` from the same menu to add the notes from a backup to the Tabs List. Imported notes receive new internal IDs, do not overwrite existing notes, and remain closed until you choose to open them from the Tabs List.

## Local data

Goatpad stores documents, settings, themes, and session data in the Windows application-data directory, normally:

```text
%LOCALAPPDATA%\Goatpad
```

Data is not synchronized or backed up automatically. Back up this directory if you want to preserve your workspace.

## Development

Useful commands:

```powershell
cargo check
cargo test
cargo fmt -- --check
```

## Current limitations

- MSI packaging currently targets 64-bit Windows
