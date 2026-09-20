# Goatpad

Goatpad is a lightweight desktop editor for Markdown and plain-text notes. It is built with Rust and egui, designed primarily for Windows, and keeps your notes stored locally.

## Features

- A Notepad-style look throughout: a custom title bar with tabs, a consolidated action bar, and a status bar
- Title-bar tabs and controls use a bottom-aligned content row with a compact, top-aligned 30px window-button group with no gaps; the active tab uses the theme primary as text with no background
- Title-bar tab padding, label size, and the close button scale automatically with the theme's font size, so tabs stay centered and unclipped across the full font-size range instead of relying on fixed margins
- A title bar subtly tinted toward each theme's secondary color, with dark themes shaded toward white and light themes toward black
- Generous ergonomic padding throughout the title bar, action bar header, status bar footer, and document editor view
- Modernized Phosphor vector iconography and native Windows-style vector window controls
- Markdown and plain-text editing in persistent tabs, with new notes defaulting to plain text (`TXT`) and switchable per note (`MD`/`TXT`)
- A unified action bar combining File/Edit/View menus, contextual Markdown formatting tools (headings, lists, bold, italic, strikethrough, link, table, clear formatting), and the note type switcher
- A searchable, hierarchical Tabs List with unlimited nested folders; notes and folders can be created, renamed, deleted, dragged between folders, and reordered by dropping above or below another item
- Inline find for the active note (`Ctrl+F`) or the entire local note collection (`Ctrl+Shift+F`), with yellow match highlighting and keyboard result navigation
- VS Code-style multiline editing: hold `Alt` and click to add secondary carets, then type, paste, indent, or delete at every caret
- The title-bar tabs strip automatically scrolls to keep the active tab visible whenever it changes, even when many tabs overflow the visible area
- Automatic note titles derived from the first line (up to 20 characters); Markdown titles omit formatting markers, while plain-text titles preserve the original line
- Import and export of all notes and their folder hierarchy through portable JSON backups
- Live Markdown highlighting, a read-only rendered Markdown preview, and common formatting actions, available from the action bar, the Edit menu, or keyboard shortcuts
- A Notepad-style status bar with smaller, reduced-opacity footer text, cursor position, character count, document type, zoom control, line-ending, and encoding; content zoom and Ctrl-based app zoom persist between launches
- Clean, padded status toasts with distinct success and error styling
- Automatic background saving
- Workspace and window restoration between launches, including DPI-safe native window size and position, the last normal bounds, and maximized state
- Tabbed Settings window with full theme CRUD (create, duplicate, edit, delete), theme-aware editor text, and separate System and Content font selections
- Configurable keyboard shortcuts
- Local storage with no account or cloud service required
- Optional in-app MSI updates: an HTTPS release manifest configured in `src/config/mod.rs` can be checked, downloaded, verified, and installed from Settings → Updates
- Spell checking powered by the same Windows Spell Checking API (`ISpellChecker`) used by Notepad: misspelled words are underlined in red as you type, and right-clicking one offers dictionary-quality replacement suggestions, "Add to dictionary", and "Ignore". Toggle it from View → Check spelling

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

The main window can be resized down to 320x210 pixels.

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

## In-app updates

Goatpad includes an in-app update flow for MSI releases. Set `UPDATE_MANIFEST_URL` in `src/config/mod.rs` to the HTTPS URL of a JSON release manifest. When an update source is configured, automatic checks are enabled by default and can be disabled in Settings. Goatpad also supports manual checks from the File menu, verifies the optional SHA-256 checksum, then closes and starts the elevated MSI upgrade.

Host each release MSI and a manifest like this on HTTPS:

```json
{
  "version": "1.2.0",
  "msi_url": "https://downloads.example.com/goatpad/Goatpad-1.2.0-x64.msi",
  "sha256": "put-the-lowercase-sha256-of-the-msi-here",
  "notes": "New features and fixes."
}
```

The `version` must be newer than the installed version. The MSI URL and manifest URL must both use HTTPS. For every release, increase `version` in `Cargo.toml`, run `build-msi.cmd`, upload the resulting MSI, calculate its SHA-256, and replace the hosted manifest.

## Keyboard shortcuts

| Action                    | Shortcut                        |
| ------------------------- | ------------------------------- |
| New tab                   | `Ctrl+N`                        |
| Close tab                 | `Ctrl+W`                        |
| Next tab                  | `Ctrl+Tab`                      |
| Previous tab              | `Ctrl+Shift+Tab`                |
| Settings                  | `Ctrl+,`                        |
| Bold / Italic / Underline | `Ctrl+B` / `Ctrl+I` / `Ctrl+U`  |
| Strikethrough             | `Ctrl+Shift+X`                  |
| Bulleted / Numbered list  | `Ctrl+Shift+8` / `Ctrl+Shift+7` |
| Insert link               | `Ctrl+K`                        |
| Find in active note       | `Ctrl+F`                        |
| Find in all notes         | `Ctrl+Shift+F`                  |
| Tabs list                 | `Ctrl+P`                        |
| MD/TXT switcher           | `Ctrl+M`                        |
| Markdown preview          | `Ctrl+O`                        |
| Close open panels         | `Escape`                        |

Formatting shortcuts only apply to Markdown notes. Find results can be traversed with `Enter` and `Shift+Enter`. `Escape` closes open panels and popups, including the find bar, Tabs List, Settings, confirmation dialogs, rename and shortcut-rebinding states. Shortcuts can be changed from the Keyboard tab in application settings, where `Reset to defaults` restores every keyboard shortcut.

When the Tabs List is open, the first visible note is selected automatically. Typing a search selects its first result; use `Up` and `Down` to move between results and `Enter` to open the selected note. Use `New folder` for a top-level folder or the `+` action on a folder for an unlimited-depth subfolder. Drag a note or folder onto a folder to move it inside; drag near the top or bottom of another row to place it before or after that item. The title-bar tabs can also be dragged to change their open-tab order.

While editing a note, `Tab` inserts an indentation tab; `Ctrl+Tab` and `Ctrl+Shift+Tab` continue to switch between tabs.

Hold `Alt` and click in the editor to add carets for multiline editing. Text insertion, paste, Enter, Tab, Backspace, and Delete are applied at every caret. A normal click or `Escape` returns to a single caret.

## The action bar

Goatpad unifies the application menus and formatting tools into a single, height-constant action bar:

- **Actions**: Tabs List, followed by the File, Edit, and View menus on the left.
- **Markdown options**: A heading dropdown (H1–H3), list dropdown (bulleted/numbered), bold, italic, strikethrough, link, table, and clear-formatting controls in the center. These tools are rendered only when the active note is Markdown (`MD`); for plain-text notes (`TXT`), the region collapses without changing the bar's height. On constrained window widths, tools collapse cleanly into a "Format" overflow menu.
- **Document switcher**: An immediate `MD`/`TXT` switch at the right edge of the bar (also switchable via the `View` menu or `Ctrl+M`). Markdown notes additionally show a preview icon beside this switcher. Activating it renders the note across the full content view and locks text editing; toggle it with `Ctrl+O`.

## Settings & Themes

The tabbed Settings window (`Ctrl+,`) contains three main tabs:

- **Themes**: View built-in (`Dark` and `Light`) and custom themes. Built-in themes are protected; you can duplicate any theme to create a new custom palette. Custom themes can be renamed, edited, applied, or deleted. Editing options include primary, secondary, and background colors, font sizing, and independent font family selection.
- **Keyboard**: Application shortcuts appear above Markdown formatting shortcuts. Rebind any hotkey by clicking an action and pressing the replacement key combination, or use `Reset to defaults` to restore the original bindings.
- **Updates**: Control automatic update checks, manually check the configured release manifest, and install available MSI releases.

The main document editor uses pure white text for dark themes and pure black text for light themes.

## Fonts

Each theme stores two independent font settings:

- **System font**: Applied to application chrome (title bar, tabs, action bar, status bar, and dialogs).
- **Content font**: Applied strictly to the note editor text area (Markdown and plain text).

Goatpad includes a catalog of 50 font choices, including Segoe UI, Georgia, Cambria, Times New Roman, Arial, Consolas, Calibri, Cascadia Code, Fira Code, Inter, JetBrains Mono, Lato, Open Sans, Raleway, Roboto, Roboto Mono, Source Code Pro, Ubuntu, and more, plus the built-in Sans and Monospace options. On Windows, Goatpad checks both machine-wide and per-user font registrations when it starts and adds only catalog fonts whose files are actually installed. This means fonts such as Inter or JetBrains Mono appear automatically after they are installed, without bundling or scanning every font on the computer. When a family has separate registered faces, Goatpad prefers its regular/normal/variable face, then medium, and does not use bold, semibold, italic, or oblique faces as the default.

Amharic and other Ethiopic-script text is supported independently of the selected content font. Goatpad prefers Windows' lighter Ebrima regular face when it is available, and bundles Abyssinica SIL as a final fallback for other systems. Both are registered in every editor font family, so selecting a font without Ethiopic glyphs no longer produces square replacement boxes. Abyssinica SIL is distributed under the SIL Open Font License; see `assets/AbyssinicaSIL-LICENSE.txt`.

## Note titles

New notes are named `Untitled` until you type on the first line. Goatpad then uses up to the first 20 characters of that line as the note title. For Markdown notes, formatting such as headings, lists, emphasis, links, and inline code is omitted from the automatic title; plain-text notes keep the line unchanged. Double-click a tab to rename it in place; press `Enter` to confirm or `Escape` to cancel. Clear the custom title to return to automatic naming.

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

Source code is organized by responsibility:

```text
src/
├── app/          # Application coordination, resources, and focused egui components
│   └── ui/       # Title/action/status bars, editor, panels, dialogs, and settings tabs
├── appearance/   # Theme model, presets, fonts, styling, and persistence
├── config/       # Static application configuration and persisted user settings
├── domain/       # Documents and workspace behavior
├── editor/       # Find, formatting, highlighting, and spell-check support
├── input/        # Keyboard actions and configurable hotkeys
├── services/     # Paths, persistence, sessions, and application updates
└── main.rs       # Thin native application bootstrap
```

Shared UI dimensions and responsive breakpoints are centralized in `src/app/ui/layout.rs`. Theme-derived border colors are defined by the appearance model; the title bar intentionally has no border.

## Current limitations

- MSI packaging currently targets 64-bit Windows
