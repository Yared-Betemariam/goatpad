# Goatpad — Implementation & Design Plan

## 1. Overview

Goatpad is a fast, native text/Markdown editor built in Rust, designed around a **zero-friction** workflow: no manual saving, tabs that persist forever until explicitly removed, and the app resuming exactly where you left off. This plan reflects the three architecture decisions made up front:

- **UI**: pure native Rust via `egui`/`eframe` — no embedded web view, smallest binary, fastest startup.
- **Platforms**: Windows first, built in a way that keeps macOS/Linux realistic later (`egui`, `eframe`, and the `directories` crate are already cross-platform, so the porting cost later is mostly packaging + a Ctrl/Cmd modifier abstraction, not a rewrite).
- **Tab model**: app-managed workspace, like Notion/Apple Notes — a new tab is immediately a real document living inside Goatpad's own storage, no file-picker friction. (You'll still be able to *import* an existing .md/.txt file into the workspace, and *export* a tab to an arbitrary location, via `rfd` file dialogs — but that's optional, not the default flow.)

## 2. Tech stack

| Purpose | Crate | Notes |
|---|---|---|
| GUI | `eframe` + `egui` | immediate-mode, single binary, no web view |
| Serialization | `serde`, `serde_json` | workspace index, session, settings |
| Markdown parsing (for highlighting) | `pulldown-cmark` | CommonMark-compliant, gives byte-offset spans via `.into_offset_iter()` |
| App data dir resolution | `directories` | cross-platform `%APPDATA%` / `~/Library/Application Support` / `~/.local/share` lookup — this is what makes the later Mac/Linux port cheap |
| Document IDs | `uuid` | stable ids independent of display title |
| File dialogs (import/export only) | `rfd` | native Windows/Mac/Linux file picker |
| Error handling | `anyhow`, `thiserror` | |

Skip pinned version numbers — run `cargo add eframe egui serde serde_json pulldown-cmark directories uuid rfd anyhow thiserror` and let Cargo resolve current versions.

## 3. Two judgment calls worth flagging

The spec left two small gaps. Here's how this plan resolves them — flag it if you want it different:

**"Underline" isn't real Markdown.** CommonMark has no underline syntax — `__text__` renders as **bold** in virtually every parser. A Ctrl+U hotkey would need to insert raw inline HTML, `<u>text</u>`. Most Markdown renderers (GitHub, VS Code preview, Obsidian) support that fine, but it's technically outside plain CommonMark. Say the word if you'd rather that hotkey do something more standard, like strikethrough (`~~text~~`).

**"Not deleted unless the user specifically asks."** Reading this as: no automatic pruning, ever — quitting the app or visually switching away from a tab never removes anything. The *only* way a document goes away is one explicit destructive action ("Delete tab"), gated behind a confirm click since — unlike a normal editor — there's no unsaved-changes safety net here to catch a misclick.

## 4. Storage layout

Everything lives under the OS app-data dir (via `directories::ProjectDirs`) — on Windows, `%APPDATA%\Goatpad\`.

```
Goatpad/
├── documents/
│   ├── 3fa2f1a0-...-b91c.md
│   ├── 9c11d4e2-...-44de.txt
│   └── ...
├── themes/
│   ├── default-dark.json
│   └── default-light.json
├── workspace.json      # tab list, order, titles, kind (md/txt)
├── session.json         # active tab, cursor/scroll per tab, window geometry
└── settings.json        # theme choice, font, keybindings, autosave timing
```

Document content files are named by UUID, not by title — the title is just metadata in `workspace.json`, so renaming a tab never touches the filesystem.

**workspace.json**
```json
{
  "version": 1,
  "tabs": [
    { "id": "3fa2f1a0-...", "title": "Untitled", "kind": "md", "created_at": "2026-08-25T10:02:00Z" },
    { "id": "9c11d4e2-...", "title": "Shopping list", "kind": "txt", "created_at": "2026-08-20T09:11:00Z" }
  ]
}
```

**session.json**
```json
{
  "active_tab": "3fa2f1a0-...",
  "window": { "width": 1000, "height": 700, "x": 120, "y": 80 },
  "tab_state": {
    "3fa2f1a0-...": { "cursor_offset": 128, "scroll_offset": 0.35 }
  }
}
```

**settings.json**
```json
{
  "theme": "default-dark",
  "font_family": "Inter",
  "font_size": 15,
  "autosave_debounce_ms": 400,
  "keybindings": {
    "bold": "Ctrl+B",
    "italic": "Ctrl+I",
    "underline": "Ctrl+U",
    "bullet_list": "Ctrl+Shift+8",
    "new_tab": "Ctrl+T",
    "delete_tab": "Ctrl+Shift+W"
  }
}
```

`eframe` also ships a built-in `Storage` trait that can auto-persist small app state (e.g. window geometry) without hand-rolled JSON. Worth using for just that piece if you want less boilerplate — but the document content and workspace index still need hand-rolled files regardless, since that's real user data you'll want to be able to inspect or back up directly.

## 5. Module structure

```
goatpad/
├── Cargo.toml
└── src/
    ├── main.rs              # entry point, eframe::run_native
    ├── app.rs                # GoatpadApp: top-level state + update loop
    ├── document.rs           # Document struct, DocKind enum
    ├── workspace.rs          # Workspace: tab list, ordering, active tab
    ├── persistence.rs        # autosave (debounced, background thread), atomic writes, load/save index
    ├── highlighting.rs       # pulldown-cmark -> egui LayoutJob
    ├── hotkeys.rs             # keybinding config, matching, formatting actions
    ├── theme.rs               # Theme struct, apply-to-egui::Visuals
    └── ui/
        ├── mod.rs
        ├── tab_bar.rs
        ├── editor.rs
        ├── footer.rs
        ├── settings_panel.rs
        └── type_switch.rs     # the MD/TXT toggle beside the filename
```

## 6. Feature notes

**Auto-save.** No save button, ever. On each edit, `Document.dirty` flips true and a debounce timer resets (~400ms of no typing → write). A dedicated background thread owns the actual file writes — the editor thread just sends `(id, content)` over an `mpsc` channel, so typing never blocks on disk I/O. Writes go to `documents/<id>.tmp` then get renamed over the real file (atomic on the same filesystem), so a crash mid-write can't corrupt a document. Also force-flush on tab switch, window focus loss, and app close.

**Tabs.** `Workspace` holds an ordered `Vec<Document>` plus an active index. "New tab" creates an empty `Document` (default kind `Md`), writes it immediately — so it survives even before the user types anything — and appends it to `workspace.json`. Drag-to-reorder tabs is a nice V2 addition, not required for V1.

**Session restore.** On launch: load `workspace.json` (which tabs exist, in what order) and `session.json` (which tab was active, exact cursor offset + scroll position per tab, window geometry). Restoring cursor position means converting a stored byte offset back into an egui `CCursorRange` on the `TextEdit` — cheap at note-taking scale.

**Footer (Ln / Col / chars).** Derived each frame from the current `TextEdit` cursor + content: split content up to the cursor offset on `\n` to get line/column, `content.chars().count()` for the character count. For very large files, recomputing the full char count every frame is wasteful — worth caching and updating incrementally on edit if you ever load big files.

**Hotkeys.** Actions (`ToggleBold`, `ToggleItalic`, `ToggleUnderline`, `ToggleBulletList`, `NewTab`, `DeleteTab`, ...) live in an enum matched against `egui::InputState` each frame using the bindings loaded from `settings.json`. Formatting actions wrap the current selection with the relevant Markdown token pair (`**`/`**`, `*`/`*`, `<u>`/`</u>`, or a `- ` prefix per selected line for lists); with no selection, they insert the empty pair and place the cursor between them. A settings panel lets the user click a binding and press a new combo to remap it — only meaningful in Md tabs, since Txt tabs carry no formatting concept.

**Themes.** `Theme { primary, secondary, background, font_family, font_size }`, applied to `egui::Visuals` on every frame the active theme changes. Multiple named presets live as small JSON files under `themes/`; `settings.json` just points at the active one. For fonts, ship a small curated set of embedded fonts (a clean sans + a monospace) rather than a full system-font picker for V1 — a system font enumerator (`font-kit`) is real added complexity for a "fast and efficient" editor, and can always be added later.

**Markdown highlighting.** `pulldown_cmark::Parser::new_ext(text, Options::all()).into_offset_iter()` walks the document giving `(Event, Range<usize>)` pairs — headings, emphasis, strong, code spans/blocks, list items, links. Map each range to a style and build an `egui::text::LayoutJob`, passed in as the `layouter` callback on `egui::TextEdit::multiline(...)`. This re-highlights live, in place, with no separate preview pane. Only active when `Document.kind == Md`; `Txt` tabs use a plain layouter with no styling. Re-parsing the whole buffer on every keystroke is fine at note-taking scale; if you ever load large files, viewport-limited highlighting is the optimization path.

**MD/TXT switch.** A small toggle beside the tab's title. Flipping it updates `Document.kind`, renames the on-disk file (`<id>.md` ⇄ `<id>.txt`), updates `workspace.json`, and turns the highlighter on/off — no confirmation needed, since content is untouched either way.

## 7. Rough UI layout

```
┌──────────────────────────────────────────────────────────┐
│ [Untitled] [Notes] [Shopping list] [+]                    │  tab bar
├──────────────────────────────────────────────────────────┤
│ Notes                                          ( MD | TXT )│  title + type switch
├──────────────────────────────────────────────────────────┤
│ # Meeting notes                                            │
│                                                              │
│ - Point one                                                 │  editor
│ - Point two                                                 │
│                                                              │
├──────────────────────────────────────────────────────────┤
│ Ln 4, Col 12   •   38 chars                                 │  footer
└──────────────────────────────────────────────────────────┘
```

## 8. Build roadmap

| Phase | Goal |
|---|---|
| 0 | `cargo new`, blank `eframe` window rendering |
| 1 | Single-document editor: one `TextEdit`, live footer (Ln/Col/chars) |
| 2 | Workspace dir + `Document`/`Workspace` model, debounced background autosave with atomic writes, load-on-launch for that one doc |
| 3 | Multi-tab: tab bar, new/switch/delete tab, full session restore (tabs, active tab, per-tab cursor + scroll) |
| 4 | Markdown highlighting (`pulldown-cmark` → `LayoutJob`) + the MD/TXT switch |
| 5 | Hotkeys: keybinding model, default bindings, bold/italic/underline/list actions, remap UI |
| 6 | Themes: `Theme` struct, apply-to-`Visuals`, embedded fonts, color-picker settings panel, multiple saved presets |
| 7 | Windows polish: icon, installer (`cargo-wix` for an MSI, or a portable `.exe`), crash-safety pass (verify a hard kill mid-type doesn't lose more than the debounce window) |
| 8 (later) | macOS/Linux: package as `.app`/AppImage, abstract Ctrl→Cmd for the primary modifier, sanity-check font rendering per platform — the core (`egui`, `eframe`, `directories`) needs no rewrite |

Build phases 0–3 first and get that loop rock-solid — autosave + session restore is the part that'll feel broken immediately if it's flaky — before touching highlighting, hotkeys, or themes.
