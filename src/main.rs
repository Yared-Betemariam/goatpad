use eframe::egui;
use std::{
    sync::mpsc::Sender,
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
use persistence::{SaveRequest, start_writer_thread};
use session::{Session, TabState, WindowGeom};
use settings::Settings;
use theme::{Theme, apply_theme, ensure_default_themes, install_fonts, load_themes, save_theme};
use workspace::Workspace;

struct GoatpadApp {
    workspace: Workspace,
    paths: AppPaths,
    session: Session,
    cursor_offset: usize,
    scroll_offset: f32,
    restore_cursor: bool,
    writer: Sender<SaveRequest>,
    last_edit: Option<Instant>,
    dirty_since: Option<Instant>,
    last_session_save: Instant,
    delete_confirmation: Option<Uuid>,
    settings: Settings,
    settings_open: bool,
    rebinding: Option<Action>,
    themes: Vec<Theme>,
    theme_draft: Theme,
    new_theme_name: String,
}

impl GoatpadApp {
    fn new(paths: AppPaths, mut session: Session, ctx: &egui::Context) -> std::io::Result<Self> {
        let mut workspace = Workspace::load(paths.clone())?;
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
        if let Some(active_tab) = session.active_tab {
            workspace.set_active_by_id(active_tab);
        }
        let active_id = workspace.active_document().id;
        let state = session
            .tab_state
            .get(&active_id)
            .copied()
            .unwrap_or_default();
        session.active_tab = Some(active_id);
        Ok(Self {
            workspace,
            paths,
            session,
            cursor_offset: state.cursor_offset,
            scroll_offset: state.scroll_offset,
            restore_cursor: true,
            writer: start_writer_thread(),
            last_edit: None,
            dirty_since: None,
            last_session_save: Instant::now(),
            delete_confirmation: None,
            settings,
            settings_open: false,
            rebinding: None,
            themes,
            theme_draft,
            new_theme_name: String::new(),
        })
    }

    fn select_theme(&mut self, ctx: &egui::Context, theme: Theme) {
        self.settings.theme = theme.name.clone();
        self.theme_draft = theme;
        apply_theme(ctx, &self.theme_draft);
        if let Err(error) = self.settings.save(&self.paths) {
            eprintln!("failed to save active theme: {error}");
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
            Err(error) => eprintln!("failed to save theme: {error}"),
        }
    }

    fn queue_active_save(&mut self) {
        let document = self.workspace.active_document();
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
        }
    }

    fn flush_active_now(&mut self) {
        let index = self.workspace.active;
        if self.workspace.documents[index].dirty {
            if let Err(error) = self
                .workspace
                .save_document(&self.workspace.documents[index])
            {
                eprintln!("failed to save document: {error}");
            } else {
                self.workspace.documents[index].dirty = false;
            }
        }
    }

    fn flush_all_now(&mut self) {
        for index in 0..self.workspace.documents.len() {
            if self.workspace.documents[index].dirty {
                if let Err(error) = self
                    .workspace
                    .save_document(&self.workspace.documents[index])
                {
                    eprintln!("failed to save document on exit: {error}");
                } else {
                    self.workspace.documents[index].dirty = false;
                }
            }
        }
    }

    fn capture_active_tab_state(&mut self) {
        let id = self.workspace.active_document().id;
        self.session.active_tab = Some(id);
        self.session.tab_state.insert(
            id,
            TabState {
                cursor_offset: self.cursor_offset,
                scroll_offset: self.scroll_offset,
            },
        );
    }

    fn switch_to(&mut self, index: usize) {
        if index == self.workspace.active {
            return;
        }
        self.capture_active_tab_state();
        self.flush_active_now();
        self.workspace.active = index;
        let state = self
            .session
            .tab_state
            .get(&self.workspace.active_document().id)
            .copied()
            .unwrap_or_default();
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
            eprintln!("failed to save session: {error}");
        } else {
            self.last_session_save = Instant::now();
        }
    }

    fn delete_tab(&mut self, id: Uuid) {
        self.capture_active_tab_state();
        self.flush_active_now();
        match self.workspace.delete_tab(id) {
            Ok(true) => {
                self.session.tab_state.remove(&id);
                let state = self
                    .session
                    .tab_state
                    .get(&self.workspace.active_document().id)
                    .copied()
                    .unwrap_or_default();
                self.cursor_offset = state.cursor_offset;
                self.scroll_offset = state.scroll_offset;
                self.restore_cursor = true;
                self.save_session();
            }
            Ok(false) => {}
            Err(error) => eprintln!("failed to delete tab: {error}"),
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
        self.capture_active_tab_state();
        self.flush_active_now();
        match self.workspace.new_tab() {
            Ok(()) => {
                self.cursor_offset = 0;
                self.scroll_offset = 0.0;
                self.restore_cursor = true;
                self.save_session();
            }
            Err(error) => eprintln!("failed to create tab: {error}"),
        }
    }

    fn editor_id(&self) -> egui::Id {
        egui::Id::new(("editor", self.workspace.active_document().id))
    }

    fn dispatch_hotkeys(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.rebinding {
            if let Some(binding) =
                ctx.input(|input| input.events.iter().find_map(hotkeys::keybinding_from_event))
            {
                self.settings.keybindings.insert(action, binding);
                if let Err(error) = self.settings.save(&self.paths) {
                    eprintln!("failed to save settings: {error}");
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
            Action::DeleteTab => {
                self.delete_confirmation = Some(self.workspace.active_document().id)
            }
            Action::OpenSettings => self.settings_open = !self.settings_open,
            action
                if action.is_formatting()
                    && self.workspace.active_document().kind == DocKind::Md =>
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
        self.workspace.active_document_mut().dirty = true;
        let now = Instant::now();
        self.last_edit = Some(now);
        self.dirty_since.get_or_insert(now);
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

impl eframe::App for GoatpadApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.update_window_geometry(&ctx);
        self.dispatch_hotkeys(&ctx);
        let mut requested_switch = None;
        let mut requested_new_tab = false;
        egui::Panel::top("tab_bar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (index, document) in self.workspace.documents.iter().enumerate() {
                    if ui
                        .selectable_label(index == self.workspace.active, &document.title)
                        .clicked()
                    {
                        requested_switch = Some(index);
                    }
                    if ui.small_button("×").on_hover_text("Delete tab").clicked() {
                        self.delete_confirmation = Some(document.id);
                    }
                }
                if ui.button("+").on_hover_text("New tab").clicked() {
                    requested_new_tab = true;
                }
                if ui.button("⚙").on_hover_text("Settings").clicked() {
                    self.settings_open = true;
                }
            });
        });
        if let Some(index) = requested_switch {
            self.switch_to(index);
        }
        if requested_new_tab {
            self.create_tab();
        }

        let (line, column) = cursor_position(
            &self.workspace.active_document().content,
            self.cursor_offset,
        );
        let character_count = self.workspace.active_document().content.chars().count();
        egui::Panel::bottom("footer").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Ln {line}, Col {column}"));
                ui.separator();
                ui.label(format!("{character_count} chars"));
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            let document_id = self.workspace.active_document().id;
            let mut requested_kind = self.workspace.active_document().kind;
            ui.horizontal(|ui| {
                ui.heading(&self.workspace.active_document().title);
                ui.separator();
                ui.selectable_value(&mut requested_kind, DocKind::Md, "MD");
                ui.selectable_value(&mut requested_kind, DocKind::Txt, "TXT");
            });
            if requested_kind != self.workspace.active_document().kind {
                self.flush_active_now();
                if let Err(error) = self
                    .workspace
                    .set_document_kind(document_id, requested_kind)
                {
                    eprintln!("failed to change document type: {error}");
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
                let now = Instant::now();
                self.workspace.active_document_mut().dirty = true;
                self.last_edit = Some(now);
                self.dirty_since.get_or_insert(now);
            }
        });

        if let Some(id) = self.delete_confirmation {
            egui::Window::new("Delete tab?")
                .collapsible(false)
                .resizable(false)
                .show(&ctx, |ui| {
                    ui.label("This permanently removes the tab and its content file.");
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            self.delete_confirmation = None;
                            self.delete_tab(id);
                        }
                        if ui.button("Cancel").clicked() {
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
        if self.workspace.active_document().dirty {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn on_exit(&mut self) {
        self.flush_all_now();
        self.save_session();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = AppPaths::new()?;
    let session = Session::load(&paths)?;
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1000.0, 700.0])
        .with_title("Goatpad");
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
