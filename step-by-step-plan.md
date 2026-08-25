# Goatpad — Step-by-Step Build Plan

Every step from an empty folder to a packaged Windows app, broken into checkable tasks. 78 steps across 9 phases (0–8). This follows the architecture in `goatpad-implementation-plan.md` — read that first if you haven't; this is the "do this, then this" version of it.

Work top to bottom. Each phase assumes the one before it is done and tested.

## Phase 0 — Project bootstrap

- [x] 0.1 Install/verify the Rust stable toolchain (`rustup`), confirm with `cargo --version`
- [x] 0.2 `cargo new goatpad --bin`
- [x] 0.3 `cargo add eframe egui`
- [x] 0.4 Replace `main.rs` with a minimal `eframe::App` impl and `run_native` call — confirm a blank window opens and closes cleanly
- [x] 0.5 `git init`, add a `.gitignore` (`/target`, etc.), first commit
- [x] 0.6 Set window defaults via `eframe::NativeOptions` (initial size, title "Goatpad")
- [x] 0.7 Confirm `cargo build --release` works; note baseline binary size and startup time
  - Baseline: 13,940,224-byte release binary; approximately 1.58 seconds to create the native window on the verification machine.

## Phase 1 — Single-document editor core

- [x] 1.1 Define a minimal `Document` struct (just `content: String` for now)
- [x] 1.2 Render one `egui::TextEdit::multiline` bound to `Document.content`, filling a `CentralPanel`
- [x] 1.3 Add a bottom `Panel` as a placeholder footer strip (`TopBottomPanel` was renamed in egui 0.36)
- [x] 1.4 Read the `TextEdit` output's cursor range and convert the char offset into line/column by scanning for `\n`
- [x] 1.5 Compute character count via `content.chars().count()`
- [x] 1.6 Wire line/col/char values into the footer, confirm they update live while typing
- [x] 1.7 Manual test: multi-line content, wrapped lines, empty file — confirm counts and cursor tracking stay correct

## Phase 2 — Persistence & autosave

- [x] 2.1 Add `directories`; write a `paths.rs` resolving `ProjectDirs::from("", "", "Goatpad")`, exposing `documents_dir()` / `themes_dir()` / config paths, creating them on first run
- [x] 2.2 Flesh out `Document`: `id: Uuid`, `title: String`, `kind: DocKind`, `content: String`, `dirty: bool`
- [x] 2.3 Define the `workspace.json` schema as serde structs (`WorkspaceIndex { version, tabs: Vec<TabEntry> }`)
- [x] 2.4 Implement `Workspace::load()` — read `workspace.json`, load each doc's content from `documents/<id>.<ext>`; if missing (first run), create one default "Untitled" tab
- [x] 2.5 Implement `Workspace::save_index()` — serialize the index to `workspace.json`
- [x] 2.6 Write a shared atomic-write helper (`<path>.tmp` then `fs::rename`) — reuse it everywhere content gets persisted
- [x] 2.7 Set up the autosave pipeline: `mpsc::channel::<SaveRequest>()` + a dedicated writer thread blocking on `recv()`
- [x] 2.8 In the update loop, detect `dirty`, reset a debounce timer (`last_edit: Instant`); once idle past ~400ms, send a `SaveRequest` and clear `dirty`
- [x] 2.9 Add a hard-cap flush (force-save if continuously dirty for >2s) so long typing bursts don't go unsaved indefinitely
- [x] 2.10 Force-flush all dirty docs synchronously on window close and on tab switch
- [x] 2.11 Crash-safety test: force-kill the process mid-typing, relaunch, confirm content matches the last debounce window with no corruption

## Phase 3 — Multi-tab + session restore

- [x] 3.1 Extend `Workspace` to hold `Vec<Document>` + an `active` index
- [x] 3.2 Build the tab bar UI (one button per tab + a `+` button)
- [x] 3.3 Wire tab click → switch `active`, flushing the previously active doc if dirty
- [x] 3.4 Wire `+` → `new_tab()`: fresh `Uuid`, default kind `Md`, title "Untitled", write immediately, append to index, switch to it
- [x] 3.5 Add a delete affordance per tab → confirm dialog → `delete_tab(id)`: remove from the list, from `workspace.json`, and delete the content file
- [x] 3.6 Define the `session.json` schema (`active_tab`, `window: WindowGeom`, `tab_state: HashMap<Uuid, TabState>` with `cursor_offset` + `scroll_offset`)
- [x] 3.7 On tab switch (and periodically), write current cursor + scroll offsets into `tab_state` and persist `session.json`
- [x] 3.8 On launch, after loading the workspace, restore the active tab and each tab's cursor position + scroll offset from `session.json`
- [x] 3.9 Capture window geometry on resize/move and persist it; apply it on launch via `NativeOptions`
- [x] 3.10 Full restart test: open 3 tabs, type in each, move cursors to different spots, resize the window, quit, relaunch — confirm everything comes back exactly

## Phase 4 — Markdown highlighting + MD/TXT switch

- [x] 4.1 Add `pulldown-cmark`; write `highlighting::highlight(text: &str) -> egui::text::LayoutJob`
- [x] 4.2 Walk `Parser::new_ext(text, Options::all()).into_offset_iter()`, collecting `(Range<usize>, TextStyle)` for headings/emphasis/strong/code/links/list markers
- [x] 4.3 Convert the span list into a `LayoutJob` (append `LayoutSection`s with the right `TextFormat`, filling gaps with default formatting)
- [x] 4.4 Wire `highlight()` as the `TextEdit`'s `.layouter()` callback, active only when `kind == Md`
- [x] 4.5 Add a plain (unstyled) fallback layouter for `Kind::Txt`
- [ ] 4.6 Visual QA: headings, bold, italic, code, lists, links each render distinctly; cursor/selection still behave normally inside styled text
- [x] 4.7 Add the MD/TXT toggle widget next to the tab title
- [x] 4.8 Wire the toggle: update `kind`, rename the file on disk (`.md` ⇄ `.txt`), update `workspace.json`, switch layouter immediately
- [x] 4.9 Test toggling mid-session and after a restart — extension change round-trips correctly, content is never touched

## Phase 5 — Hotkeys

- [ ] 5.1 Define an `Action` enum: `ToggleBold`, `ToggleItalic`, `ToggleUnderline`, `ToggleBulletList`, `NewTab`, `DeleteTab`, etc.
- [ ] 5.2 Define a `Keybinding` struct (`egui::Key` + `egui::Modifiers`) with serialization to/from strings like `"Ctrl+B"`
- [ ] 5.3 Ship a default `HashMap<Action, Keybinding>`, overridable by `settings.json`
- [ ] 5.4 Each frame, check `ctx.input()` against active bindings and dispatch the matching `Action`
- [ ] 5.5 Implement formatting-wrap logic: selection gets wrapped in the token pair (`**`/`**`, `*`/`*`, `<u>`/`</u>`); no selection inserts the empty pair with cursor between
- [ ] 5.6 Implement list-toggle: prefix each selected line with `- ` (strip it if already present)
- [ ] 5.7 Gate formatting actions to `Kind::Md` tabs only
- [ ] 5.8 Build the settings panel's keybinding list with a "press new combo" rebind capture mode
- [ ] 5.9 Persist rebindings to `settings.json` immediately on change
- [ ] 5.10 Test every hotkey individually, plus rebinding one and confirming it takes effect without a restart

## Phase 6 — Themes

- [ ] 6.1 Define `Theme { name, primary, secondary, background, font_family, font_size }` with hex-color serde support
- [ ] 6.2 Write `apply_theme(ctx, theme)` to set `egui::Visuals` fields (fills, selection color, widget colors) from the theme
- [ ] 6.3 Ship 1–2 default themes as `themes/default-dark.json` / `default-light.json`
- [ ] 6.4 Load the active theme from `settings.json` on launch and apply it once at startup
- [ ] 6.5 Embed a small curated font set (`include_bytes!` + `FontDefinitions`) — one sans, one monospace
- [ ] 6.6 Build the theme settings panel: color pickers for primary/secondary/background, a font dropdown, live preview
- [ ] 6.7 Implement "save as new theme" — writes a new file under `themes/`, adds it to a picker list
- [ ] 6.8 Implement theme switching: picking a saved theme updates `settings.json` and re-applies immediately
- [ ] 6.9 Test: create a custom theme, switch away and back, restart, confirm it persists

## Phase 7 — Windows polish & packaging

- [ ] 7.1 Add an app icon (`.ico`), wire via `NativeOptions::icon_data` + a `build.rs`/`winres` step for the exe icon
- [ ] 7.2 Set the window title to reflect the active tab (e.g. "Notes — Goatpad")
- [ ] 7.3 Handle true first-launch (no `workspace.json`) by auto-creating one default tab
- [ ] 7.4 Keyboard-only pass: create/switch/delete tabs and reach settings without touching the mouse
- [ ] 7.5 Error-handling pass: read-only `documents/`, full disk, missing/corrupted content file — fail loud via an in-app toast rather than losing data silently
- [ ] 7.6 Measure cold-start time and typing latency with several tabs open; profile if either feels off
- [ ] 7.7 Package a release build — try `cargo-wix` for an MSI, or ship a portable `.exe` + short README as a first pass
- [ ] 7.8 Re-run the crash-safety test on the packaged build itself, not just `cargo run`
- [ ] 7.9 Write a short internal changelog/README of current features and known gaps, to anchor Phase 8 later

## Phase 8 — Cross-platform (later, once Windows is solid)

- [ ] 8.1 Abstract the primary modifier key (Ctrl on Win/Linux vs Cmd on macOS) in the default bindings table
- [ ] 8.2 Verify `directories::ProjectDirs` paths resolve sensibly on macOS/Linux
- [ ] 8.3 macOS: build a proper `.app` bundle with a native menu bar
- [ ] 8.4 Linux: package as an AppImage or `.deb`, sanity-check on at least one desktop environment
- [ ] 8.5 Cross-platform font-rendering check (sizing/hinting differences)
- [ ] 8.6 Re-run the full crash-safety and session-restore test suite on each new platform
