#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::{
    sync::mpsc::{Receiver, Sender, TryRecvError},
    time::{Duration, Instant},
};
use uuid::Uuid;

mod document;
mod highlighting;
mod hotkeys;
mod paths;
mod persistence;
mod session;
mod settings;
mod theme;
mod workspace;

use document::DocKind;
use hotkeys::{Action, Keybinding};
use paths::AppPaths;
use persistence::{SaveRequest, SaveResult, start_writer_thread};
use session::{Session, TabState, WindowGeom};
use settings::Settings;
use theme::{Theme, apply_theme, ensure_default_themes, install_fonts, load_themes, save_theme};
use workspace::Workspace;

const TITLE_BAR_HEIGHT: f32 = 40.0;
const TITLE_BAR_SPACING: f32 = 4.0;
const TITLE_CONTROL_WIDTH: f32 = 32.0;
const WINDOW_BUTTON_WIDTH: f32 = 46.0;
const MIN_DRAG_WIDTH: f32 = 48.0;
const RESIZE_BORDER_WIDTH: f32 = 5.0;
const RESIZE_CORNER_SIZE: f32 = 14.0;

#[derive(Clone, Copy)]
enum ToastKind {
    Error,
    Success,
}

struct Toast {
    message: String,
    shown_at: Instant,
    kind: ToastKind,
}

struct GoatpadApp {
    workspace: Workspace,
    paths: AppPaths,
    session: Session,
    cursor_offset: usize,
    scroll_offset: f32,
    restore_cursor: bool,
    writer: Sender<SaveRequest>,
    writer_results: Receiver<SaveResult>,
    last_edit: Option<Instant>,
    dirty_since: Option<Instant>,
    last_session_save: Instant,
    delete_confirmation: Option<Uuid>,
    tabs_list_open: bool,
    tabs_list_search: String,
    settings: Settings,
    settings_open: bool,
    rebinding: Option<Action>,
    themes: Vec<Theme>,
    theme_draft: Theme,
    new_theme_name: String,
    renaming_document: Option<Uuid>,
    rename_buffer: String,
    focus_rename: bool,
    workspace_index_dirty: bool,
    toasts: Vec<Toast>,
}

impl GoatpadApp {
    fn new(paths: AppPaths, mut session: Session, ctx: &egui::Context) -> std::io::Result<Self> {
        let (mut workspace, startup_warnings) = Workspace::load(paths.clone())?;
        let settings = Settings::load(&paths)?;
        ensure_default_themes(&paths)?;
        let themes = load_themes(&paths)?;
        let theme_draft = themes
            .iter()
            .find(|theme| theme.name == settings.theme)
            .cloned()
            .unwrap_or_else(Theme::default_dark);
        install_fonts(ctx);
        apply_theme(ctx, &theme_draft);
        let note_ids = workspace
            .documents
            .iter()
            .map(|document| document.id)
            .collect::<Vec<_>>();
        session.prepare_open_tabs(&note_ids);
        let state = if let Some(active_id) = session.active_tab {
            workspace.set_active_by_id(active_id);
            workspace.touch_document(active_id)?;
            session
                .tab_state
                .get(&active_id)
                .copied()
                .unwrap_or_default()
        } else {
            TabState::default()
        };
        session.save(&paths)?;
        let (writer, writer_results) = start_writer_thread();
        Ok(Self {
            workspace,
            paths,
            session,
            cursor_offset: state.cursor_offset,
            scroll_offset: state.scroll_offset,
            restore_cursor: true,
            writer,
            writer_results,
            last_edit: None,
            dirty_since: None,
            last_session_save: Instant::now(),
            delete_confirmation: None,
            tabs_list_open: false,
            tabs_list_search: String::new(),
            settings,
            settings_open: false,
            rebinding: None,
            themes,
            theme_draft,
            new_theme_name: String::new(),
            renaming_document: None,
            rename_buffer: String::new(),
            focus_rename: false,
            workspace_index_dirty: false,
            toasts: startup_warnings
                .into_iter()
                .map(|message| Toast {
                    message,
                    shown_at: Instant::now(),
                    kind: ToastKind::Error,
                })
                .collect(),
        })
    }

    fn report_error(&mut self, message: impl Into<String>) {
        self.toasts.push(Toast {
            message: message.into(),
            shown_at: Instant::now(),
            kind: ToastKind::Error,
        });
    }

    fn report_success(&mut self, message: impl Into<String>) {
        self.toasts.push(Toast {
            message: message.into(),
            shown_at: Instant::now(),
            kind: ToastKind::Success,
        });
    }

    fn poll_writer_results(&mut self) {
        loop {
            match self.writer_results.try_recv() {
                Ok(SaveResult {
                    id: _,
                    result: Ok(()),
                }) => {}
                Ok(SaveResult {
                    id,
                    result: Err(error),
                }) => {
                    if let Some(document) =
                        self.workspace.documents.iter_mut().find(|doc| doc.id == id)
                    {
                        document.dirty = true;
                    }
                    self.last_edit = Some(Instant::now());
                    self.report_error(error);
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
    }

    fn select_theme(&mut self, ctx: &egui::Context, theme: Theme) {
        self.settings.theme = theme.name.clone();
        self.theme_draft = theme;
        apply_theme(ctx, &self.theme_draft);
        if let Err(error) = self.settings.save(&self.paths) {
            self.report_error(format!("Could not save the active theme: {error}"));
        }
    }

    fn save_new_theme(&mut self, ctx: &egui::Context) {
        let name = self.new_theme_name.trim();
        if name.is_empty() {
            return;
        }
        let mut theme = self.theme_draft.clone();
        theme.name = name.to_owned();
        match save_theme(&self.paths, &theme) {
            Ok(()) => {
                if let Some(existing) = self
                    .themes
                    .iter_mut()
                    .find(|saved| saved.name == theme.name)
                {
                    *existing = theme.clone();
                } else {
                    self.themes.push(theme.clone());
                    self.themes
                        .sort_by(|left, right| left.name.cmp(&right.name));
                }
                self.new_theme_name.clear();
                self.select_theme(ctx, theme);
            }
            Err(error) => self.report_error(format!("Could not save theme: {error}")),
        }
    }

    fn begin_rename(&mut self, id: Uuid) {
        if let Some(document) = self
            .workspace
            .documents
            .iter()
            .find(|document| document.id == id)
        {
            self.renaming_document = Some(id);
            self.rename_buffer = document.title.clone();
            self.focus_rename = true;
        }
    }

    fn finish_rename(&mut self) {
        let Some(id) = self.renaming_document.take() else {
            return;
        };
        match self.workspace.rename_document(id, &self.rename_buffer) {
            Ok(()) => self.workspace_index_dirty = false,
            Err(error) => {
                self.workspace_index_dirty = true;
                self.report_error(format!("Could not rename note: {error}"));
            }
        }
        self.rename_buffer.clear();
        self.focus_rename = false;
    }

    fn cancel_rename(&mut self) {
        self.renaming_document = None;
        self.rename_buffer.clear();
        self.focus_rename = false;
    }

    fn mark_active_document_edited(&mut self) {
        let title_changed = {
            let document = self.workspace.active_document_mut();
            document.dirty = true;
            document.refresh_automatic_title()
        };
        self.workspace_index_dirty |= title_changed;
        let now = Instant::now();
        self.last_edit = Some(now);
        self.dirty_since.get_or_insert(now);
    }

    fn flush_workspace_index(&mut self) {
        if !self.workspace_index_dirty {
            return;
        }
        match self.workspace.save_index() {
            Ok(()) => self.workspace_index_dirty = false,
            Err(error) => self.report_error(format!("Could not save note titles: {error}")),
        }
    }

    fn export_notes(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export Goatpad notes")
            .add_filter("Goatpad notes", &["json"])
            .set_file_name("goatpad-notes.json")
            .save_file()
        else {
            return;
        };
        if self.renaming_document.is_some() {
            self.finish_rename();
        }
        self.flush_all_now();
        match self.workspace.export_notes(&path) {
            Ok(()) => self.report_success(format!(
                "Exported {} notes to {}",
                self.workspace.documents.len(),
                path.display()
            )),
            Err(error) => self.report_error(format!("Could not export notes: {error}")),
        }
    }

    fn import_notes(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import Goatpad notes")
            .add_filter("Goatpad notes", &["json"])
            .pick_file()
        else {
            return;
        };
        if self.renaming_document.is_some() {
            self.finish_rename();
        }
        self.capture_active_tab_state();
        self.flush_all_now();
        match self.workspace.import_notes(&path) {
            Ok(count) => {
                self.workspace_index_dirty = false;
                self.save_session();
                self.report_success(format!("Imported {count} notes from {}", path.display()));
            }
            Err(error) => self.report_error(format!("Could not import notes: {error}")),
        }
    }

    fn queue_active_save(&mut self) {
        let Some(active_id) = self.session.active_tab else {
            return;
        };
        let Some(document) = self.workspace.document(active_id) else {
            return;
        };
        if !document.dirty {
            return;
        }
        let request = SaveRequest {
            id: document.id,
            kind: document.kind,
            content: document.content.clone(),
            path: self.workspace.document_path(document.id, document.kind),
        };
        if self.writer.send(request).is_ok() {
            self.workspace.active_document_mut().dirty = false;
            self.last_edit = None;
            self.dirty_since = None;
            self.flush_workspace_index();
        }
    }

    fn flush_active_now(&mut self) {
        let Some(active_id) = self.session.active_tab else {
            self.flush_workspace_index();
            return;
        };
        let Some(index) = self
            .workspace
            .documents
            .iter()
            .position(|document| document.id == active_id)
        else {
            self.flush_workspace_index();
            return;
        };
        if self.workspace.documents[index].dirty {
            if let Err(error) = self
                .workspace
                .save_document(&self.workspace.documents[index])
            {
                self.report_error(format!("Could not save document: {error}"));
            } else {
                self.workspace.documents[index].dirty = false;
            }
        }
        self.flush_workspace_index();
    }

    fn flush_all_now(&mut self) {
        for index in 0..self.workspace.documents.len() {
            if self.workspace.documents[index].dirty {
                if let Err(error) = self
                    .workspace
                    .save_document(&self.workspace.documents[index])
                {
                    self.report_error(format!("Could not save document: {error}"));
                } else {
                    self.workspace.documents[index].dirty = false;
                }
            }
        }
        self.flush_workspace_index();
    }

    fn capture_active_tab_state(&mut self) {
        let Some(id) = self.session.active_tab else {
            return;
        };
        self.session.tab_state.insert(
            id,
            TabState {
                cursor_offset: self.cursor_offset,
                scroll_offset: self.scroll_offset,
            },
        );
    }

    fn activate_tab(&mut self, id: Uuid) {
        if self.workspace.document(id).is_none() {
            return;
        }
        if self.renaming_document.is_some() {
            self.finish_rename();
        }
        let changed = self.session.active_tab != Some(id);
        if changed {
            self.capture_active_tab_state();
            self.flush_active_now();
        }
        self.session.open_tab(id);
        self.workspace.set_active_by_id(id);
        if let Err(error) = self.workspace.touch_document(id) {
            self.report_error(format!("Could not update note activity: {error}"));
        }
        let state = self.session.tab_state.get(&id).copied().unwrap_or_default();
        self.cursor_offset = state.cursor_offset;
        self.scroll_offset = state.scroll_offset;
        self.restore_cursor = true;
        self.last_edit = None;
        self.dirty_since = None;
        self.save_session();
    }

    fn save_session(&mut self) {
        self.capture_active_tab_state();
        if let Err(error) = self.session.save(&self.paths) {
            self.report_error(format!("Could not save session: {error}"));
        } else {
            self.last_session_save = Instant::now();
        }
    }

    fn close_tab(&mut self, id: Uuid) {
        if !self.session.open_tabs.contains(&id) {
            return;
        }
        if self.renaming_document == Some(id) {
            self.finish_rename();
        }
        let was_active = self.session.active_tab == Some(id);
        if was_active {
            self.capture_active_tab_state();
            self.flush_active_now();
        }
        self.session.close_tab(id);
        if was_active {
            if let Some(next_id) = self.session.active_tab {
                self.workspace.set_active_by_id(next_id);
                if let Err(error) = self.workspace.touch_document(next_id) {
                    self.report_error(format!("Could not update note activity: {error}"));
                }
                let state = self
                    .session
                    .tab_state
                    .get(&next_id)
                    .copied()
                    .unwrap_or_default();
                self.cursor_offset = state.cursor_offset;
                self.scroll_offset = state.scroll_offset;
                self.restore_cursor = true;
            } else {
                self.cursor_offset = 0;
                self.scroll_offset = 0.0;
                self.restore_cursor = false;
            }
            self.last_edit = None;
            self.dirty_since = None;
        }
        self.save_session();
    }

    fn delete_note(&mut self, id: Uuid) {
        if self.session.open_tabs.contains(&id) {
            self.close_tab(id);
        }
        match self.workspace.delete_note(id) {
            Ok(true) => {
                self.session.tab_state.remove(&id);
                self.save_session();
            }
            Ok(false) => {}
            Err(error) => self.report_error(format!("Could not delete note: {error}")),
        }
    }

    fn update_window_geometry(&mut self, ctx: &egui::Context) {
        if let Some(rect) = ctx.input(|input| input.viewport().outer_rect) {
            self.session.window = Some(WindowGeom {
                width: rect.width(),
                height: rect.height(),
                x: rect.left(),
                y: rect.top(),
            });
        }
    }

    fn create_tab(&mut self) {
        if self.renaming_document.is_some() {
            self.finish_rename();
        }
        self.capture_active_tab_state();
        self.flush_active_now();
        match self.workspace.new_tab() {
            Ok(id) => {
                self.session.open_tab(id);
                self.workspace.set_active_by_id(id);
                self.cursor_offset = 0;
                self.scroll_offset = 0.0;
                self.restore_cursor = true;
                self.last_edit = None;
                self.dirty_since = None;
                self.save_session();
            }
            Err(error) => self.report_error(format!("Could not create tab: {error}")),
        }
    }

    fn editor_id(&self) -> egui::Id {
        egui::Id::new((
            "editor",
            self.session
                .active_tab
                .expect("editor requires an active tab"),
        ))
    }

    fn dispatch_hotkeys(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.rebinding {
            if let Some(binding) =
                ctx.input(|input| input.events.iter().find_map(hotkeys::keybinding_from_event))
            {
                self.settings.keybindings.insert(action, binding);
                if let Err(error) = self.settings.save(&self.paths) {
                    self.report_error(format!("Could not save keyboard shortcuts: {error}"));
                }
                self.rebinding = None;
            }
            return;
        }
        let action = Action::ALL.into_iter().find(|action| {
            self.settings
                .keybindings
                .get(action)
                .is_some_and(|binding| {
                    ctx.input_mut(|input| input.consume_key(binding.modifiers, binding.key))
                })
        });
        let Some(action) = action else {
            return;
        };
        match action {
            Action::NewTab => self.create_tab(),
            Action::CloseTab => {
                if let Some(id) = self.session.active_tab {
                    self.close_tab(id);
                }
            }
            Action::NextTab => {
                let previous = self.session.active_tab;
                if let Some(id) = self.session.cycle_tab(true) {
                    self.session.active_tab = previous;
                    self.activate_tab(id);
                }
            }
            Action::PreviousTab => {
                let previous = self.session.active_tab;
                if let Some(id) = self.session.cycle_tab(false) {
                    self.session.active_tab = previous;
                    self.activate_tab(id);
                }
            }
            Action::OpenSettings => self.settings_open = !self.settings_open,
            action
                if action.is_formatting()
                    && self
                        .session
                        .active_tab
                        .and_then(|id| self.workspace.document(id))
                        .is_some_and(|document| document.kind == DocKind::Md) =>
            {
                self.apply_formatting(ctx, action);
            }
            _ => {}
        }
    }

    fn apply_formatting(&mut self, ctx: &egui::Context, action: Action) {
        let editor_id = self.editor_id();
        let range = egui::widgets::text_edit::TextEditState::load(ctx, editor_id)
            .and_then(|state| state.cursor.char_range())
            .unwrap_or_else(|| {
                egui::text::CCursorRange::one(egui::text::CCursor::new(self.cursor_offset))
            });
        let (start, end) = if range.primary.index.0 <= range.secondary.index.0 {
            (range.primary.index.0, range.secondary.index.0)
        } else {
            (range.secondary.index.0, range.primary.index.0)
        };
        let new_range = if action == Action::ToggleBulletList {
            toggle_bullet_list(
                &mut self.workspace.active_document_mut().content,
                start,
                end,
            )
        } else {
            let (open, close) = match action {
                Action::ToggleBold => ("**", "**"),
                Action::ToggleItalic => ("*", "*"),
                Action::ToggleUnderline => ("<u>", "</u>"),
                _ => return,
            };
            wrap_selection(
                &mut self.workspace.active_document_mut().content,
                start,
                end,
                open,
                close,
            )
        };
        let mut state =
            egui::widgets::text_edit::TextEditState::load(ctx, editor_id).unwrap_or_default();
        state.cursor.set_char_range(Some(new_range));
        state.store(ctx, editor_id);
        self.cursor_offset = new_range.primary.index.0;
        self.mark_active_document_edited();
    }
}

fn byte_index(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(index, _)| index)
}

fn wrap_selection(
    text: &mut String,
    start: usize,
    end: usize,
    open: &str,
    close: &str,
) -> egui::text::CCursorRange {
    let start = start.min(text.chars().count());
    let end = end.min(text.chars().count());
    let start_byte = byte_index(text, start);
    let end_byte = byte_index(text, end);
    text.insert_str(end_byte, close);
    text.insert_str(start_byte, open);
    if start == end {
        egui::text::CCursorRange::one(egui::text::CCursor::new(start + open.chars().count()))
    } else {
        egui::text::CCursorRange::two(
            egui::text::CCursor::new(start + open.chars().count()),
            egui::text::CCursor::new(end + open.chars().count()),
        )
    }
}

fn toggle_bullet_list(text: &mut String, start: usize, end: usize) -> egui::text::CCursorRange {
    let start_byte = byte_index(text, start.min(text.chars().count()));
    let end_byte = byte_index(text, end.min(text.chars().count()));
    let line_start = text[..start_byte].rfind('\n').map_or(0, |index| index + 1);
    let line_end = text[end_byte..]
        .find('\n')
        .map_or(text.len(), |index| end_byte + index);
    let selected = &text[line_start..line_end];
    let lines: Vec<&str> = selected.split('\n').collect();
    let remove = lines.iter().all(|line| line.starts_with("- "));
    let replacement = lines
        .into_iter()
        .map(|line| {
            if remove {
                line.strip_prefix("- ").unwrap_or(line).to_owned()
            } else {
                format!("- {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let replacement_chars = replacement.chars().count();
    text.replace_range(line_start..line_end, &replacement);
    let prefix_chars = text[..line_start].chars().count();
    egui::text::CCursorRange::two(
        egui::text::CCursor::new(prefix_chars),
        egui::text::CCursor::new(prefix_chars + replacement_chars),
    )
}

fn cursor_position(content: &str, cursor_offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for character in content.chars().take(cursor_offset) {
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn show_app_icon_menu(
    ui: &mut egui::Ui,
    requested_import: &mut bool,
    requested_export: &mut bool,
    settings_open: &mut bool,
) {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(TITLE_CONTROL_WIDTH, TITLE_BAR_HEIGHT),
        egui::Sense::click(),
    );
    if response.hovered() || response.is_pointer_button_down_on() {
        let fill = if response.is_pointer_button_down_on() {
            ui.style().visuals.widgets.active.bg_fill
        } else {
            ui.style().visuals.widgets.hovered.bg_fill
        };
        ui.painter().rect_filled(rect, 0.0, fill);
    }

    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(20.0, 22.0));
    ui.painter()
        .rect_filled(icon_rect, 2.0, egui::Color32::from_rgb(28, 52, 40));
    let page_rect = icon_rect.shrink2(egui::vec2(3.0, 2.0));
    ui.painter()
        .rect_filled(page_rect, 1.0, egui::Color32::from_rgb(107, 193, 123));
    for offset in [6.0, 10.0, 14.0] {
        let right_padding = if offset == 14.0 { 5.0 } else { 2.0 };
        ui.painter().line_segment(
            [
                egui::pos2(page_rect.left() + 2.0, page_rect.top() + offset),
                egui::pos2(page_rect.right() - right_padding, page_rect.top() + offset),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_rgb(231, 255, 235)),
        );
    }

    let response = response.on_hover_text("Goatpad menu");
    egui::Popup::menu(&response).show(|ui| {
        if ui.button("Import notes…").clicked() {
            *requested_import = true;
            ui.close();
        }
        if ui.button("Export notes…").clicked() {
            *requested_export = true;
            ui.close();
        }
        ui.separator();
        if ui.button("Settings").clicked() {
            *settings_open = true;
            ui.close();
        }
    });
}

fn window_button(ui: &mut egui::Ui, label: &str, is_close: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(WINDOW_BUTTON_WIDTH, TITLE_BAR_HEIGHT),
        egui::Sense::click(),
    );
    let pointer_down = response.is_pointer_button_down_on();
    let fill = if is_close && (response.hovered() || pointer_down) {
        egui::Color32::from_rgb(196, 43, 28)
    } else if pointer_down {
        ui.style().visuals.widgets.active.bg_fill
    } else if response.hovered() {
        ui.style().visuals.widgets.hovered.bg_fill
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    let text_color = if is_close && (response.hovered() || pointer_down) {
        egui::Color32::WHITE
    } else {
        ui.style().visuals.text_color()
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::TextStyle::Button.resolve(ui.style()),
        text_color,
    );
    response
}

fn show_resize_handles(ctx: &egui::Context) {
    if ctx.input(|input| input.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let viewport = ctx.viewport_rect();
    if viewport.width() <= 2.0 * RESIZE_CORNER_SIZE || viewport.height() <= 2.0 * RESIZE_CORNER_SIZE
    {
        return;
    }

    let zones = [
        (
            "north",
            egui::Rect::from_min_max(
                egui::pos2(viewport.left() + RESIZE_CORNER_SIZE, viewport.top()),
                egui::pos2(
                    viewport.right() - RESIZE_CORNER_SIZE,
                    viewport.top() + RESIZE_BORDER_WIDTH,
                ),
            ),
            egui::ResizeDirection::North,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            "south",
            egui::Rect::from_min_max(
                egui::pos2(
                    viewport.left() + RESIZE_CORNER_SIZE,
                    viewport.bottom() - RESIZE_BORDER_WIDTH,
                ),
                egui::pos2(viewport.right() - RESIZE_CORNER_SIZE, viewport.bottom()),
            ),
            egui::ResizeDirection::South,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            "west",
            egui::Rect::from_min_max(
                egui::pos2(viewport.left(), viewport.top() + RESIZE_CORNER_SIZE),
                egui::pos2(
                    viewport.left() + RESIZE_BORDER_WIDTH,
                    viewport.bottom() - RESIZE_CORNER_SIZE,
                ),
            ),
            egui::ResizeDirection::West,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            "east",
            egui::Rect::from_min_max(
                egui::pos2(
                    viewport.right() - RESIZE_BORDER_WIDTH,
                    viewport.top() + RESIZE_CORNER_SIZE,
                ),
                egui::pos2(viewport.right(), viewport.bottom() - RESIZE_CORNER_SIZE),
            ),
            egui::ResizeDirection::East,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            "north_west",
            egui::Rect::from_min_size(viewport.left_top(), egui::Vec2::splat(RESIZE_CORNER_SIZE)),
            egui::ResizeDirection::NorthWest,
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            "north_east",
            egui::Rect::from_min_size(
                egui::pos2(viewport.right() - RESIZE_CORNER_SIZE, viewport.top()),
                egui::Vec2::splat(RESIZE_CORNER_SIZE),
            ),
            egui::ResizeDirection::NorthEast,
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            "south_west",
            egui::Rect::from_min_size(
                egui::pos2(viewport.left(), viewport.bottom() - RESIZE_CORNER_SIZE),
                egui::Vec2::splat(RESIZE_CORNER_SIZE),
            ),
            egui::ResizeDirection::SouthWest,
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            "south_east",
            egui::Rect::from_min_size(
                egui::pos2(
                    viewport.right() - RESIZE_CORNER_SIZE,
                    viewport.bottom() - RESIZE_CORNER_SIZE,
                ),
                egui::Vec2::splat(RESIZE_CORNER_SIZE),
            ),
            egui::ResizeDirection::SouthEast,
            egui::CursorIcon::ResizeNwSe,
        ),
    ];

    for (name, zone, direction, cursor) in zones {
        egui::Area::new(egui::Id::new(("viewport_resize", name)))
            .order(egui::Order::Foreground)
            .fixed_pos(zone.min)
            .constrain(false)
            .show(ctx, |ui| {
                let response = ui
                    .allocate_response(zone.size(), egui::Sense::drag())
                    .on_hover_cursor(cursor);
                if response.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
                }
            });
    }
}

fn goatpad_icon() -> egui::IconData {
    const SIZE: usize = 64;
    let mut rgba = vec![0_u8; SIZE * SIZE * 4];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let pixel = (y * SIZE + x) * 4;
            let inside_page = (7..57).contains(&x) && (6..59).contains(&y);
            let (red, green, blue) = if inside_page {
                (107, 193, 123)
            } else {
                (28, 52, 40)
            };
            rgba[pixel..pixel + 4].copy_from_slice(&[red, green, blue, 255]);
        }
    }
    for y in [20_usize, 32, 44] {
        for x in 19..if y == 44 { 43 } else { 48 } {
            let pixel = (y * SIZE + x) * 4;
            rgba[pixel..pixel + 4].copy_from_slice(&[231, 255, 235, 255]);
        }
    }
    egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
}

impl eframe::App for GoatpadApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_writer_results();
        self.update_window_geometry(&ctx);
        self.dispatch_hotkeys(&ctx);
        let mut requested_switch = None;
        let mut requested_close = None;
        let mut requested_rename = None;
        let mut requested_new_tab = false;
        let mut requested_import = false;
        let mut requested_export = false;
        let tabs = self
            .session
            .open_tabs
            .iter()
            .filter_map(|id| {
                self.workspace
                    .document(*id)
                    .map(|document| (*id, document.title.clone()))
            })
            .collect::<Vec<_>>();
        egui::Panel::top("title_bar")
            .exact_size(TITLE_BAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(ui.style().visuals.panel_fill)
                    .inner_margin(egui::Margin::ZERO),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(TITLE_BAR_SPACING, 0.0);
                ui.horizontal(|ui| {
                    ui.set_height(TITLE_BAR_HEIGHT);
                    show_app_icon_menu(
                        ui,
                        &mut requested_import,
                        &mut requested_export,
                        &mut self.settings_open,
                    );

                    if !tabs.is_empty() {
                        let fixed_width = 2.0 * TITLE_CONTROL_WIDTH
                            + 3.0 * WINDOW_BUTTON_WIDTH
                            + MIN_DRAG_WIDTH
                            + 7.0 * TITLE_BAR_SPACING;
                        let tabs_width = (ui.available_width() - fixed_width).max(80.0);
                        egui::ScrollArea::horizontal()
                            .id_salt("title_bar_tabs")
                            .max_width(tabs_width)
                            .scroll_bar_visibility(
                                egui::containers::scroll_area::ScrollBarVisibility::AlwaysHidden,
                            )
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    for (id, title) in &tabs {
                                        let response = ui
                                            .selectable_label(
                                                self.session.active_tab == Some(*id),
                                                title,
                                            )
                                            .on_hover_text("Double-click to rename");
                                        if response.clicked() {
                                            requested_switch = Some(*id);
                                        }
                                        if response.double_clicked() {
                                            requested_rename = Some(*id);
                                        }
                                        let close_response =
                                            ui.small_button("×").on_hover_text("Close tab");
                                        if response.middle_clicked()
                                            || close_response.middle_clicked()
                                            || close_response.clicked()
                                        {
                                            requested_close = Some(*id);
                                        }
                                    }
                                });
                            });
                    }

                    if ui
                        .add_sized(
                            [TITLE_CONTROL_WIDTH, TITLE_CONTROL_WIDTH],
                            egui::Button::new("+").frame(false),
                        )
                        .on_hover_text("New tab")
                        .clicked()
                    {
                        requested_new_tab = true;
                    }
                    if ui
                        .add_sized(
                            [TITLE_CONTROL_WIDTH, TITLE_CONTROL_WIDTH],
                            egui::Button::new("⋯").frame(false),
                        )
                        .on_hover_text("Tabs list")
                        .clicked()
                    {
                        self.tabs_list_open = !self.tabs_list_open;
                    }

                    let drag_width = (ui.available_width()
                        - 3.0 * WINDOW_BUTTON_WIDTH
                        - 3.0 * TITLE_BAR_SPACING)
                        .max(0.0);
                    if drag_width > 0.0 {
                        let drag_response = ui
                            .allocate_response(
                                egui::vec2(drag_width, TITLE_BAR_HEIGHT),
                                egui::Sense::drag(),
                            )
                            .on_hover_cursor(egui::CursorIcon::Grab);
                        if drag_response.drag_started_by(egui::PointerButton::Primary) {
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }
                    }

                    if window_button(ui, "−", false)
                        .on_hover_text("Minimize")
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
                    let maximize_label = if maximized { "❐" } else { "□" };
                    if window_button(ui, maximize_label, false)
                        .on_hover_text(if maximized { "Restore" } else { "Maximize" })
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }
                    if window_button(ui, "×", true)
                        .on_hover_text("Close")
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        if let Some(id) = requested_switch {
            self.activate_tab(id);
        }
        if let Some(id) = requested_close {
            self.close_tab(id);
        }
        if let Some(id) = requested_rename {
            self.begin_rename(id);
        }
        if requested_new_tab {
            self.create_tab();
        }
        if requested_import {
            self.import_notes();
        }
        if requested_export {
            self.export_notes();
        }

        let mut requested_list_open = None;
        let mut requested_list_delete = None;
        if self.tabs_list_open {
            let mut notes = self
                .workspace
                .documents
                .iter()
                .map(|document| {
                    (
                        document.id,
                        document.title.clone(),
                        document.last_opened_at,
                        self.session.open_tabs.contains(&document.id),
                    )
                })
                .collect::<Vec<_>>();
            notes.sort_by(|left, right| right.2.cmp(&left.2));
            let mut list_open = self.tabs_list_open;
            egui::Window::new("Tabs list")
                .open(&mut list_open)
                .collapsible(false)
                .resizable(true)
                .default_width(360.0)
                .show(&ctx, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.tabs_list_search)
                            .hint_text("Search notes…")
                            .desired_width(f32::INFINITY),
                    );
                    ui.separator();
                    let query = self.tabs_list_search.trim().to_lowercase();
                    egui::ScrollArea::vertical()
                        .max_height(420.0)
                        .show(ui, |ui| {
                            for (id, title, _, is_open) in &notes {
                                if !query.is_empty() && !title.to_lowercase().contains(&query) {
                                    continue;
                                }
                                ui.horizontal(|ui| {
                                    ui.label(if *is_open { "●" } else { " " }).on_hover_text(
                                        if *is_open {
                                            "Open in the tab bar"
                                        } else {
                                            "Not open"
                                        },
                                    );
                                    if ui
                                        .selectable_label(false, title)
                                        .on_hover_text("Open note")
                                        .clicked()
                                    {
                                        requested_list_open = Some(*id);
                                    }
                                    if ui
                                        .small_button("🗑")
                                        .on_hover_text("Delete note permanently")
                                        .clicked()
                                    {
                                        requested_list_delete = Some(*id);
                                    }
                                });
                            }
                            if notes.is_empty() {
                                ui.label("No notes saved.");
                            }
                        });
                });
            self.tabs_list_open = list_open;
        }
        if let Some(id) = requested_list_open {
            self.tabs_list_open = false;
            self.activate_tab(id);
        }
        if let Some(id) = requested_list_delete {
            self.delete_confirmation = Some(id);
        }

        let editor_stats = self.session.active_tab.and_then(|id| {
            self.workspace.document(id).map(|document| {
                (
                    cursor_position(&document.content, self.cursor_offset),
                    document.content.chars().count(),
                )
            })
        });
        egui::Panel::bottom("footer").show(ui, |ui| {
            if let Some(((line, column), character_count)) = editor_stats {
                ui.horizontal(|ui| {
                    ui.label(format!("Ln {line}, Col {column}"));
                    ui.separator();
                    ui.label(format!("{character_count} chars"));
                });
            } else {
                ui.label("No tab open");
            }
        });

        egui::CentralPanel::default().show(ui, |ui| {
            let Some(document_id) = self.session.active_tab else {
                ui.centered_and_justified(|ui| {
                    ui.label("No tabs open — create a new note, or pick one from the tabs list.");
                });
                return;
            };
            if !self.workspace.set_active_by_id(document_id) {
                ui.centered_and_justified(|ui| {
                    ui.label("No tabs open — create a new note, or pick one from the tabs list.");
                });
                return;
            }
            let mut requested_kind = self.workspace.active_document().kind;
            let mut requested_title_rename = false;
            let mut finish_rename = false;
            let mut cancel_rename = false;
            ui.horizontal(|ui| {
                if self.renaming_document == Some(document_id) {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.rename_buffer)
                            .desired_width(280.0)
                            .hint_text("Note title"),
                    );
                    if self.focus_rename {
                        response.request_focus();
                        self.focus_rename = false;
                    }
                    let enter = response.has_focus()
                        && ui.input_mut(|input| {
                            input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                        });
                    cancel_rename = response.has_focus()
                        && ui.input_mut(|input| {
                            input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                        });
                    finish_rename = !cancel_rename && (enter || response.lost_focus());
                } else {
                    requested_title_rename = ui
                        .add(
                            egui::Label::new(
                                egui::RichText::new(&self.workspace.active_document().title)
                                    .heading(),
                            )
                            .sense(egui::Sense::click()),
                        )
                        .on_hover_text("Double-click to rename")
                        .double_clicked();
                }
                ui.separator();
                ui.selectable_value(&mut requested_kind, DocKind::Md, "MD");
                ui.selectable_value(&mut requested_kind, DocKind::Txt, "TXT");
            });
            if cancel_rename {
                self.cancel_rename();
            } else if finish_rename {
                self.finish_rename();
            } else if requested_title_rename {
                self.begin_rename(document_id);
            }
            if requested_kind != self.workspace.active_document().kind {
                self.flush_active_now();
                if let Err(error) = self
                    .workspace
                    .set_document_kind(document_id, requested_kind)
                {
                    self.report_error(format!("Could not change document type: {error}"));
                }
            }
            let is_markdown = self.workspace.active_document().kind == DocKind::Md;
            let editor_id = self.editor_id();
            let mut layouter =
                move |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, wrap_width: f32| {
                    let mut job = if is_markdown {
                        highlighting::highlight(buffer.as_str())
                    } else {
                        highlighting::plain(buffer.as_str())
                    };
                    job.wrap.max_width = wrap_width;
                    ui.fonts_mut(|fonts| fonts.layout_job(job))
                };
            let output = egui::ScrollArea::vertical()
                .id_salt(("editor-scroll", document_id))
                .vertical_scroll_offset(self.scroll_offset)
                .show(ui, |ui| {
                    egui::TextEdit::multiline(&mut self.workspace.active_document_mut().content)
                        .id(editor_id)
                        .desired_width(f32::INFINITY)
                        .layouter(&mut layouter)
                        .show(ui)
                });
            self.scroll_offset = output.state.offset.y;
            let editor = output.inner;
            if self.restore_cursor {
                let offset = self
                    .cursor_offset
                    .min(self.workspace.active_document().content.chars().count());
                let mut state =
                    egui::widgets::text_edit::TextEditState::load(&ctx, editor.response.id)
                        .unwrap_or_default();
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(
                        egui::text::CCursor::new(offset),
                    )));
                state.store(&ctx, editor.response.id);
                self.restore_cursor = false;
            }
            if let Some(cursor_range) = editor.cursor_range {
                self.cursor_offset = cursor_range.primary.index.0;
            }
            if editor.response.changed() {
                self.mark_active_document_edited();
            }
        });

        if let Some(id) = self.delete_confirmation {
            let title = self
                .workspace
                .document(id)
                .map_or("Untitled", |document| document.title.as_str())
                .to_owned();
            let cancel_with_keyboard =
                ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            egui::Window::new("Delete note?")
                .collapsible(false)
                .resizable(false)
                .show(&ctx, |ui| {
                    ui.label(format!(
                        "Delete \"{title}\" permanently? This cannot be undone."
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            self.delete_confirmation = None;
                            self.delete_note(id);
                        }
                        if cancel_with_keyboard || ui.button("Cancel").clicked() {
                            self.delete_confirmation = None;
                        }
                    });
                });
        }

        if self.settings_open {
            let mut settings_open = self.settings_open;
            egui::Window::new("Settings")
                .open(&mut settings_open)
                .resizable(true)
                .show(&ctx, |ui| {
                    ui.heading("Theme");
                    let selected_theme = self.settings.theme.clone();
                    egui::ComboBox::from_label("Saved theme")
                        .selected_text(&selected_theme)
                        .show_ui(ui, |ui| {
                            for theme in self.themes.clone() {
                                if ui
                                    .selectable_label(theme.name == selected_theme, &theme.name)
                                    .clicked()
                                {
                                    self.select_theme(&ctx, theme);
                                }
                            }
                        });
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.label("Primary");
                        changed |= ui
                            .color_edit_button_srgba(&mut self.theme_draft.primary.0)
                            .changed();
                        ui.label("Secondary");
                        changed |= ui
                            .color_edit_button_srgba(&mut self.theme_draft.secondary.0)
                            .changed();
                        ui.label("Background");
                        changed |= ui
                            .color_edit_button_srgba(&mut self.theme_draft.background.0)
                            .changed();
                    });
                    changed |= egui::ComboBox::from_label("Font")
                        .selected_text(&self.theme_draft.font_family)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.theme_draft.font_family,
                                "Sans".to_owned(),
                                "Sans",
                            );
                            ui.selectable_value(
                                &mut self.theme_draft.font_family,
                                "Monospace".to_owned(),
                                "Monospace",
                            );
                        })
                        .response
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut self.theme_draft.font_size, 12.0..=24.0)
                                .text("Font size"),
                        )
                        .changed();
                    if changed {
                        apply_theme(&ctx, &self.theme_draft);
                    }
                    ui.horizontal(|ui| {
                        ui.label("Save as");
                        ui.text_edit_singleline(&mut self.new_theme_name);
                        if ui.button("Save new theme").clicked() {
                            self.save_new_theme(&ctx);
                        }
                    });
                    ui.separator();
                    ui.heading("Keyboard shortcuts");
                    ui.label("Click a shortcut, then press its replacement key combination.");
                    egui::Grid::new("keybinding_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            for action in Action::ALL {
                                ui.label(action.label());
                                let text = if self.rebinding == Some(action) {
                                    "Press new combo…".to_owned()
                                } else {
                                    self.settings
                                        .keybindings
                                        .get(&action)
                                        .map_or_else(|| "Unbound".to_owned(), Keybinding::to_string)
                                };
                                if ui.button(text).clicked() {
                                    self.rebinding = Some(action);
                                }
                                ui.end_row();
                            }
                        });
                });
            self.settings_open = settings_open;
        }

        let now = Instant::now();
        self.toasts
            .retain(|toast| now.duration_since(toast.shown_at) < Duration::from_secs(8));
        if !self.toasts.is_empty() {
            egui::Area::new(egui::Id::new("status_toasts"))
                .anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -12.0])
                .show(&ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        for toast in &self.toasts {
                            let color = match toast.kind {
                                ToastKind::Error => egui::Color32::from_rgb(235, 105, 105),
                                ToastKind::Success => egui::Color32::from_rgb(107, 193, 123),
                            };
                            ui.colored_label(color, &toast.message);
                        }
                    });
                });
        }

        if self
            .last_edit
            .is_some_and(|last| now.duration_since(last) >= Duration::from_millis(400))
            || self
                .dirty_since
                .is_some_and(|since| now.duration_since(since) >= Duration::from_secs(2))
        {
            self.queue_active_save();
        }
        if now.duration_since(self.last_session_save) >= Duration::from_secs(1) {
            self.save_session();
        }
        if self
            .session
            .active_tab
            .and_then(|id| self.workspace.document(id))
            .is_some_and(|document| document.dirty)
        {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        show_resize_handles(&ctx);
    }

    fn on_exit(&mut self) {
        if self.renaming_document.is_some() {
            self.finish_rename();
        }
        self.flush_all_now();
        self.save_session();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = AppPaths::new()?;
    let session = Session::load(&paths)?;
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1000.0, 700.0])
        .with_min_inner_size([640.0, 420.0])
        .with_title("Goatpad")
        .with_icon(goatpad_icon())
        .with_decorations(false);
    if let Some(window) = session.window {
        viewport = viewport
            .with_inner_size([window.width, window.height])
            .with_position([window.x, window.y]);
    }
    eframe::run_native(
        "Goatpad",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |creation_context| {
            Ok(Box::new(GoatpadApp::new(
                paths,
                session,
                &creation_context.egui_ctx,
            )?))
        }),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{cursor_position, toggle_bullet_list, wrap_selection};
    #[test]
    fn cursor_starts_at_line_one_column_one() {
        assert_eq!(cursor_position("", 0), (1, 1));
    }
    #[test]
    fn cursor_position_tracks_newlines() {
        assert_eq!(cursor_position("first\nsecond", 6), (2, 1));
    }
    #[test]
    fn cursor_position_counts_unicode_characters() {
        assert_eq!(cursor_position("café\n🦀", 6), (2, 2));
    }

    #[test]
    fn formatting_wraps_a_selection_and_leaves_it_selected() {
        let mut text = "hello world".to_owned();
        let range = wrap_selection(&mut text, 6, 11, "**", "**");
        assert_eq!(text, "hello **world**");
        assert_eq!(
            (
                range.primary.index.0.min(range.secondary.index.0),
                range.primary.index.0.max(range.secondary.index.0)
            ),
            (8, 13)
        );
    }

    #[test]
    fn list_toggle_adds_then_removes_each_selected_line() {
        let mut text = "one\ntwo".to_owned();
        let length = text.chars().count();
        toggle_bullet_list(&mut text, 0, length);
        assert_eq!(text, "- one\n- two");
        let length = text.chars().count();
        toggle_bullet_list(&mut text, 0, length);
        assert_eq!(text, "one\ntwo");
    }
}
