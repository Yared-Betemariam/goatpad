#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::{
    sync::mpsc::{Receiver, Sender, TryRecvError},
    time::{Duration, Instant},
};
use uuid::Uuid;

mod document;
mod formatting;
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
use theme::{
    FONT_OPTIONS, Theme, apply_theme, ensure_default_themes, install_fonts, load_themes, save_theme,
};
use workspace::Workspace;

const TITLE_BAR_HEIGHT: f32 = 42.0;
const ACTION_BAR_HEIGHT: f32 = 42.0;
const TITLE_BAR_SPACING: f32 = 6.0;
const TITLE_CONTROL_WIDTH: f32 = 32.0;
const WINDOW_BUTTON_WIDTH: f32 = 46.0;
const MIN_DRAG_WIDTH: f32 = 48.0;
const RESIZE_BORDER_WIDTH: f32 = 5.0;
const RESIZE_CORNER_SIZE: f32 = 14.0;
const APP_ICON_SIZE: usize = 64;
const APP_ICON_RGBA: &[u8; APP_ICON_SIZE * APP_ICON_SIZE * 4] =
    include_bytes!("../assets/icon.rgba");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SettingsTab {
    #[default]
    Themes,
    Keyboard,
}

impl SettingsTab {
    const ALL: &[Self] = &[Self::Themes, Self::Keyboard];

    fn title(self) -> &'static str {
        match self {
            Self::Themes => "Themes",
            Self::Keyboard => "Keyboard",
        }
    }
}

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
    app_icon_texture: egui::TextureHandle,
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
    settings_tab: SettingsTab,
    rebinding: Option<Action>,
    themes: Vec<Theme>,
    theme_draft: Theme,
    title_bar_color: egui::Color32,
    editing_theme: Option<Theme>,
    editing_theme_is_new: bool,
    theme_delete_confirm: Option<String>,
    renaming_document: Option<Uuid>,
    rename_buffer: String,
    focus_rename: bool,
    workspace_index_dirty: bool,
    toasts: Vec<Toast>,
    zoom: f32,
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
        let app_icon_texture = ctx.load_texture(
            "goatpad-app-icon",
            egui::ColorImage::from_rgba_unmultiplied([APP_ICON_SIZE, APP_ICON_SIZE], APP_ICON_RGBA),
            egui::TextureOptions::LINEAR,
        );
        Ok(Self {
            app_icon_texture,
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
            settings_tab: SettingsTab::default(),
            rebinding: None,
            themes,
            title_bar_color: theme_draft.title_bar_color(),
            theme_draft,
            editing_theme: None,
            editing_theme_is_new: false,
            theme_delete_confirm: None,
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
            zoom: 1.0,
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
        self.apply_theme(ctx, &self.theme_draft.clone());
        if let Err(error) = self.settings.save(&self.paths) {
            self.report_error(format!("Could not save the active theme: {error}"));
        }
    }

    fn apply_theme(&mut self, ctx: &egui::Context, theme: &Theme) {
        apply_theme(ctx, theme);
        self.title_bar_color = theme.title_bar_color();
    }

    fn start_create_theme(&mut self) {
        let mut new_theme = self.theme_draft.clone();
        new_theme.name = "Custom Theme".to_owned();
        self.editing_theme = Some(new_theme);
        self.editing_theme_is_new = true;
    }

    fn duplicate_theme(&mut self, base: &Theme) {
        let mut clone = base.clone();
        clone.name = format!("{} Copy", base.name);
        self.editing_theme = Some(clone);
        self.editing_theme_is_new = true;
    }

    fn save_editing_theme(&mut self, ctx: &egui::Context) {
        let Some(mut theme) = self.editing_theme.take() else {
            return;
        };
        let trimmed_name = theme.name.trim().to_owned();
        if trimmed_name.is_empty() {
            self.report_error("Theme name cannot be empty");
            self.editing_theme = Some(theme);
            return;
        }
        theme.name = trimmed_name;

        if let Err(error) = save_theme(&self.paths, &theme) {
            self.report_error(format!("Could not save theme: {error}"));
            self.editing_theme = Some(theme);
            return;
        }

        if let Some(existing) = self.themes.iter_mut().find(|t| t.name == theme.name) {
            *existing = theme.clone();
        } else {
            self.themes.push(theme.clone());
            self.themes.sort_by(|a, b| a.name.cmp(&b.name));
        }

        if self.editing_theme_is_new || self.settings.theme == theme.name {
            self.select_theme(ctx, theme);
        }
        self.editing_theme_is_new = false;
        self.report_success("Theme saved");
    }

    fn delete_custom_theme(&mut self, ctx: &egui::Context, theme_name: &str) {
        if theme_name == "default-dark" || theme_name == "default-light" {
            self.report_error("Built-in themes cannot be deleted");
            return;
        }
        match theme::delete_theme(&self.paths, theme_name) {
            Ok(true) => {
                self.themes.retain(|t| t.name != theme_name);
                self.report_success(format!("Deleted theme \"{theme_name}\""));
                if self.settings.theme == theme_name {
                    let fallback = self
                        .themes
                        .iter()
                        .find(|t| t.name == "default-light")
                        .cloned()
                        .unwrap_or_else(Theme::default_light);
                    self.select_theme(ctx, fallback);
                }
            }
            Ok(false) => {
                self.report_error(format!("Theme \"{theme_name}\" not found"));
            }
            Err(error) => {
                self.report_error(format!("Could not delete theme: {error}"));
            }
        }
    }

    fn render_themes_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if let Some(mut draft) = self.editing_theme.take() {
            ui.horizontal(|ui| {
                if ui
                    .button(format!(
                        "{} Back to themes",
                        egui_phosphor::regular::ARROW_LEFT
                    ))
                    .clicked()
                {
                    self.apply_theme(ctx, &self.theme_draft.clone());
                    return;
                }
                ui.heading(if self.editing_theme_is_new {
                    "New Theme"
                } else {
                    "Edit Theme"
                });
            });
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.add(egui::TextEdit::singleline(&mut draft.name).desired_width(180.0));
            });

            ui.add_space(6.0);
            ui.label(egui::RichText::new("Colors").strong());
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("Primary");
                changed |= ui.color_edit_button_srgba(&mut draft.primary.0).changed();
                ui.label("Secondary");
                changed |= ui.color_edit_button_srgba(&mut draft.secondary.0).changed();
                ui.label("Background");
                changed |= ui
                    .color_edit_button_srgba(&mut draft.background.0)
                    .changed();
            });

            ui.add_space(6.0);
            ui.label(egui::RichText::new("Typography").strong());
            egui::ComboBox::from_label("System font")
                .selected_text(&draft.system_font)
                .show_ui(ui, |ui| {
                    for font in FONT_OPTIONS {
                        changed |= ui
                            .selectable_value(&mut draft.system_font, (*font).to_owned(), *font)
                            .changed();
                    }
                });

            egui::ComboBox::from_label("Content font")
                .selected_text(&draft.content_font)
                .show_ui(ui, |ui| {
                    for font in FONT_OPTIONS {
                        changed |= ui
                            .selectable_value(&mut draft.content_font, (*font).to_owned(), *font)
                            .changed();
                    }
                });

            changed |= ui
                .add(egui::Slider::new(&mut draft.font_size, 12.0..=24.0).text("Font size"))
                .changed();

            if changed {
                self.apply_theme(ctx, &draft);
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    self.editing_theme = Some(draft);
                    self.save_editing_theme(ctx);
                } else if ui.button("Cancel").clicked() {
                    self.apply_theme(ctx, &self.theme_draft.clone());
                } else {
                    self.editing_theme = Some(draft);
                }
            });
            return;
        }

        ui.horizontal(|ui| {
            ui.heading("Available themes");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(format!("{} New Theme", egui_phosphor::regular::PLUS))
                    .clicked()
                {
                    self.start_create_theme();
                }
            });
        });

        if let Some(to_delete) = self.theme_delete_confirm.clone() {
            ui.group(|ui| {
                ui.label(format!("Delete custom theme \"{to_delete}\"?"));
                ui.horizontal(|ui| {
                    if ui.button("Confirm Delete").clicked() {
                        self.theme_delete_confirm = None;
                        self.delete_custom_theme(ctx, &to_delete);
                    }
                    if ui.button("Cancel").clicked() {
                        self.theme_delete_confirm = None;
                    }
                });
            });
        }

        egui::ScrollArea::vertical()
            .max_height(350.0)
            .show(ui, |ui| {
                let themes = self.themes.clone();
                for theme in themes {
                    let is_active = theme.name == self.settings.theme;
                    let is_builtin = theme.is_builtin();
                    ui.horizontal(|ui| {
                        let badge = if is_active {
                            format!("{} ", egui_phosphor::regular::CHECK)
                        } else {
                            "   ".to_owned()
                        };
                        ui.label(badge);
                        ui.label(egui::RichText::new(theme.display_name()).strong());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !is_builtin {
                                if ui
                                    .small_button(egui_phosphor::regular::TRASH)
                                    .on_hover_text("Delete theme")
                                    .clicked()
                                {
                                    self.theme_delete_confirm = Some(theme.name.clone());
                                }
                                if ui
                                    .small_button("Edit")
                                    .on_hover_text("Edit colors and fonts")
                                    .clicked()
                                {
                                    self.editing_theme = Some(theme.clone());
                                    self.editing_theme_is_new = false;
                                }
                            }
                            if ui
                                .small_button("Duplicate")
                                .on_hover_text("Duplicate as new custom theme")
                                .clicked()
                            {
                                self.duplicate_theme(&theme);
                            }
                            if !is_active {
                                if ui
                                    .small_button("Apply")
                                    .on_hover_text("Apply this theme")
                                    .clicked()
                                {
                                    self.select_theme(ctx, theme.clone());
                                }
                            } else {
                                ui.label(egui::RichText::new("Active").weak());
                            }
                        });
                    });
                    ui.separator();
                }
            });
    }

    fn render_keyboard_settings(&mut self, ui: &mut egui::Ui) {
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

    fn selection_range(&self, ctx: &egui::Context) -> (usize, usize) {
        let editor_id = self.editor_id();
        let range = egui::widgets::text_edit::TextEditState::load(ctx, editor_id)
            .and_then(|state| state.cursor.char_range())
            .unwrap_or_else(|| {
                egui::text::CCursorRange::one(egui::text::CCursor::new(self.cursor_offset))
            });
        if range.primary.index.0 <= range.secondary.index.0 {
            (range.primary.index.0, range.secondary.index.0)
        } else {
            (range.secondary.index.0, range.primary.index.0)
        }
    }

    /// Applies a selection-transforming edit (formatting, list toggles, links, tables, …)
    /// to the active document and restores the cursor/selection afterwards.
    fn apply_text_transform(
        &mut self,
        ctx: &egui::Context,
        transform: impl FnOnce(&mut String, usize, usize) -> egui::text::CCursorRange,
    ) {
        let (start, end) = self.selection_range(ctx);
        let editor_id = self.editor_id();
        let new_range = transform(
            &mut self.workspace.active_document_mut().content,
            start,
            end,
        );
        let mut state =
            egui::widgets::text_edit::TextEditState::load(ctx, editor_id).unwrap_or_default();
        state.cursor.set_char_range(Some(new_range));
        state.store(ctx, editor_id);
        self.cursor_offset = new_range.primary.index.0;
        self.mark_active_document_edited();
    }

    fn apply_formatting(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::ToggleBulletList => {
                self.apply_text_transform(ctx, formatting::toggle_bullet_list)
            }
            Action::ToggleNumberedList => {
                self.apply_text_transform(ctx, formatting::toggle_numbered_list)
            }
            Action::InsertLink => self.apply_text_transform(ctx, formatting::insert_link),
            Action::ToggleBold => {
                self.apply_text_transform(ctx, |text, start, end| {
                    formatting::wrap_selection(text, start, end, "**", "**")
                });
            }
            Action::ToggleItalic => {
                self.apply_text_transform(ctx, |text, start, end| {
                    formatting::wrap_selection(text, start, end, "*", "*")
                });
            }
            Action::ToggleUnderline => {
                self.apply_text_transform(ctx, |text, start, end| {
                    formatting::wrap_selection(text, start, end, "<u>", "</u>")
                });
            }
            Action::ToggleStrikethrough => {
                self.apply_text_transform(ctx, |text, start, end| {
                    formatting::wrap_selection(text, start, end, "~~", "~~")
                });
            }
            _ => {}
        }
    }

    fn apply_heading(&mut self, ctx: &egui::Context, level: u8) {
        self.apply_text_transform(ctx, move |text, start, end| {
            formatting::set_heading(text, start, end, level)
        });
    }

    fn apply_clear_formatting(&mut self, ctx: &egui::Context) {
        self.apply_text_transform(ctx, formatting::clear_formatting);
    }

    fn apply_table_insert(&mut self, ctx: &egui::Context) {
        self.apply_text_transform(ctx, formatting::insert_table);
    }
}

/// Paints the small application icon shown at the left of the title bar.
/// It is purely decorative, matching Notepad's non-interactive app icon.
fn show_app_icon(ui: &mut egui::Ui, app_icon_texture: &egui::TextureHandle) {
    let (rect, _response) = ui.allocate_exact_size(
        egui::vec2(TITLE_CONTROL_WIDTH, TITLE_BAR_HEIGHT),
        egui::Sense::hover(),
    );
    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(22.0));
    ui.painter().image(
        app_icon_texture.id(),
        icon_rect,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WindowControlKind {
    Minimize,
    Maximize,
    Restore,
    Close,
}

fn window_control_button(ui: &mut egui::Ui, kind: WindowControlKind) -> egui::Response {
    let is_close = kind == WindowControlKind::Close;
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
    let stroke_color = if is_close && (response.hovered() || pointer_down) {
        egui::Color32::WHITE
    } else {
        ui.style().visuals.text_color()
    };
    let center = rect.center();

    match kind {
        WindowControlKind::Minimize => {
            let cy = center.y.round() + 0.5;
            let cx = center.x.round();
            ui.painter().line_segment(
                [egui::pos2(cx - 5.0, cy), egui::pos2(cx + 5.0, cy)],
                egui::Stroke::new(1.0, stroke_color),
            );
        }
        WindowControlKind::Maximize => {
            let box_rect = egui::Rect::from_center_size(center, egui::vec2(10.0, 10.0));
            ui.painter().rect_stroke(
                box_rect,
                0.0,
                egui::Stroke::new(1.0, stroke_color),
                egui::StrokeKind::Middle,
            );
        }
        WindowControlKind::Restore => {
            let back =
                egui::Rect::from_min_size(center + egui::vec2(-2.5, -4.5), egui::vec2(7.0, 7.0));
            ui.painter().rect_stroke(
                back,
                0.0,
                egui::Stroke::new(1.0, stroke_color),
                egui::StrokeKind::Middle,
            );
            let front =
                egui::Rect::from_min_size(center + egui::vec2(-4.5, -2.5), egui::vec2(7.0, 7.0));
            let bg_fill = if fill == egui::Color32::TRANSPARENT {
                ui.style().visuals.panel_fill
            } else {
                fill
            };
            ui.painter().rect_filled(front, 0.0, bg_fill);
            ui.painter().rect_stroke(
                front,
                0.0,
                egui::Stroke::new(1.0, stroke_color),
                egui::StrokeKind::Middle,
            );
        }
        WindowControlKind::Close => {
            let d = 4.5;
            ui.painter().line_segment(
                [
                    egui::pos2(center.x - d, center.y - d),
                    egui::pos2(center.x + d, center.y + d),
                ],
                egui::Stroke::new(1.2, stroke_color),
            );
            ui.painter().line_segment(
                [
                    egui::pos2(center.x - d, center.y + d),
                    egui::pos2(center.x + d, center.y - d),
                ],
                egui::Stroke::new(1.2, stroke_color),
            );
        }
    }
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
    egui::IconData {
        rgba: APP_ICON_RGBA.to_vec(),
        width: APP_ICON_SIZE as u32,
        height: APP_ICON_SIZE as u32,
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
        let mut finish_tab_rename = false;
        let mut cancel_tab_rename = false;
        let active_is_markdown = self
            .session
            .active_tab
            .and_then(|id| self.workspace.document(id))
            .is_some_and(|document| document.kind == DocKind::Md);
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
                    .fill(self.title_bar_color)
                    .inner_margin(egui::Margin {
                        left: 10,
                        right: 0,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(TITLE_BAR_SPACING, 0.0);
                ui.horizontal(|ui| {
                    ui.set_height(TITLE_BAR_HEIGHT);
                    show_app_icon(ui, &self.app_icon_texture);

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
                                        if self.renaming_document == Some(*id) {
                                            let response = ui.add(
                                                egui::TextEdit::singleline(&mut self.rename_buffer)
                                                    .desired_width(140.0)
                                                    .hint_text("Note title"),
                                            );
                                            if self.focus_rename {
                                                response.request_focus();
                                                self.focus_rename = false;
                                            }
                                            let enter = response.has_focus()
                                                && ui.input_mut(|input| {
                                                    input.consume_key(
                                                        egui::Modifiers::NONE,
                                                        egui::Key::Enter,
                                                    )
                                                });
                                            let escape = response.has_focus()
                                                && ui.input_mut(|input| {
                                                    input.consume_key(
                                                        egui::Modifiers::NONE,
                                                        egui::Key::Escape,
                                                    )
                                                });
                                            if escape {
                                                cancel_tab_rename = true;
                                            } else if enter || response.lost_focus() {
                                                finish_tab_rename = true;
                                            }
                                        } else {
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
                                            let close_response = ui
                                                .add(
                                                    egui::Button::new(egui_phosphor::regular::X)
                                                        .frame(false)
                                                        .min_size(egui::vec2(18.0, 18.0)),
                                                )
                                                .on_hover_text("Close tab");
                                            if response.middle_clicked()
                                                || close_response.middle_clicked()
                                                || close_response.clicked()
                                            {
                                                requested_close = Some(*id);
                                            }
                                        }
                                    }
                                });
                            });
                    }

                    if ui
                        .add_sized(
                            [TITLE_CONTROL_WIDTH, TITLE_CONTROL_WIDTH],
                            egui::Button::new(egui_phosphor::regular::PLUS).frame(false),
                        )
                        .on_hover_text("New tab")
                        .clicked()
                    {
                        requested_new_tab = true;
                    }
                    if ui
                        .add_sized(
                            [TITLE_CONTROL_WIDTH, TITLE_CONTROL_WIDTH],
                            egui::Button::new(egui_phosphor::regular::LIST).frame(false),
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

                    if window_control_button(ui, WindowControlKind::Minimize)
                        .on_hover_text("Minimize")
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
                    let max_kind = if maximized {
                        WindowControlKind::Restore
                    } else {
                        WindowControlKind::Maximize
                    };
                    if window_control_button(ui, max_kind)
                        .on_hover_text(if maximized { "Restore" } else { "Maximize" })
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }
                    if window_control_button(ui, WindowControlKind::Close)
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
        if cancel_tab_rename {
            self.cancel_rename();
        } else if finish_tab_rename {
            self.finish_rename();
        }

        egui::Panel::top("action_bar")
            .exact_size(ACTION_BAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(ui.style().visuals.panel_fill)
                    .inner_margin(egui::Margin {
                        left: 14,
                        right: 14,
                        top: 4,
                        bottom: 4,
                    }),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                ui.spacing_mut().button_padding = egui::vec2(7.0, 4.0);
                ui.horizontal(|ui| {
                    ui.set_height(ACTION_BAR_HEIGHT - 8.0);

                    // Region 1: Actions
                    ui.menu_button("File", |ui| {
                        if ui.button("New tab").clicked() {
                            self.create_tab();
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                self.session.active_tab.is_some(),
                                egui::Button::new("Close tab"),
                            )
                            .clicked()
                        {
                            if let Some(id) = self.session.active_tab {
                                self.close_tab(id);
                            }
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Tabs list").clicked() {
                            self.tabs_list_open = true;
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Import notes").clicked() {
                            self.import_notes();
                            ui.close();
                        }
                        if ui.button("Export notes").clicked() {
                            self.export_notes();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Settings").clicked() {
                            self.settings_open = true;
                            ui.close();
                        }
                    });
                    ui.menu_button("Edit", |ui| {
                        let enabled = active_is_markdown;
                        if ui
                            .add_enabled(enabled, egui::Button::new("Bold"))
                            .on_hover_text("Ctrl+B")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::ToggleBold);
                            ui.close();
                        }
                        if ui
                            .add_enabled(enabled, egui::Button::new("Italic"))
                            .on_hover_text("Ctrl+I")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::ToggleItalic);
                            ui.close();
                        }
                        if ui
                            .add_enabled(enabled, egui::Button::new("Underline"))
                            .on_hover_text("Ctrl+U")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::ToggleUnderline);
                            ui.close();
                        }
                        if ui
                            .add_enabled(enabled, egui::Button::new("Strikethrough"))
                            .on_hover_text("Ctrl+Shift+X")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::ToggleStrikethrough);
                            ui.close();
                        }
                        ui.separator();
                        if ui
                            .add_enabled(enabled, egui::Button::new("Bulleted list"))
                            .on_hover_text("Ctrl+Shift+8")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::ToggleBulletList);
                            ui.close();
                        }
                        if ui
                            .add_enabled(enabled, egui::Button::new("Numbered list"))
                            .on_hover_text("Ctrl+Shift+7")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::ToggleNumberedList);
                            ui.close();
                        }
                        ui.separator();
                        if ui
                            .add_enabled(enabled, egui::Button::new("Link…"))
                            .on_hover_text("Ctrl+K")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::InsertLink);
                            ui.close();
                        }
                        if ui
                            .add_enabled(enabled, egui::Button::new("Clear formatting"))
                            .clicked()
                        {
                            self.apply_clear_formatting(&ctx);
                            ui.close();
                        }
                    });
                    ui.menu_button("View", |ui| {
                        if let Some(document_id) = self.session.active_tab {
                            let current_kind = self
                                .workspace
                                .document(document_id)
                                .map(|document| document.kind);
                            if ui
                                .selectable_label(
                                    current_kind == Some(DocKind::Md),
                                    "Markdown document",
                                )
                                .clicked()
                            {
                                self.flush_active_now();
                                if let Err(error) =
                                    self.workspace.set_document_kind(document_id, DocKind::Md)
                                {
                                    self.report_error(format!(
                                        "Could not change document type: {error}"
                                    ));
                                }
                                ui.close();
                            }
                            if ui
                                .selectable_label(
                                    current_kind == Some(DocKind::Txt),
                                    "Plain text document",
                                )
                                .clicked()
                            {
                                self.flush_active_now();
                                if let Err(error) =
                                    self.workspace.set_document_kind(document_id, DocKind::Txt)
                                {
                                    self.report_error(format!(
                                        "Could not change document type: {error}"
                                    ));
                                }
                                ui.close();
                            }
                            ui.separator();
                        }
                        ui.menu_button("Theme", |ui| {
                            for theme in self.themes.clone() {
                                if ui
                                    .selectable_label(
                                        theme.name == self.settings.theme,
                                        theme.display_name(),
                                    )
                                    .clicked()
                                {
                                    self.select_theme(&ctx, theme);
                                    ui.close();
                                }
                            }
                        });
                        ui.separator();
                        if ui.button("Zoom in").clicked() {
                            self.zoom = (self.zoom + 0.1).min(3.0);
                            ui.close();
                        }
                        if ui.button("Zoom out").clicked() {
                            self.zoom = (self.zoom - 0.1).max(0.5);
                            ui.close();
                        }
                        if ui.button("Reset zoom").clicked() {
                            self.zoom = 1.0;
                            ui.close();
                        }
                    });

                    // Region 2: Markdown options (only rendered when active note is MD)
                    if active_is_markdown {
                        ui.separator();
                        let available_width = ui.available_width();
                        if available_width < 380.0 {
                            ui.menu_button("Format", |ui| {
                                ui.menu_button(
                                    format!("{} Headings", egui_phosphor::regular::TEXT_H),
                                    |ui| {
                                        if ui
                                            .button(format!(
                                                "{} Heading 1",
                                                egui_phosphor::regular::TEXT_H_ONE
                                            ))
                                            .clicked()
                                        {
                                            self.apply_heading(&ctx, 1);
                                            ui.close();
                                        }
                                        if ui
                                            .button(format!(
                                                "{} Heading 2",
                                                egui_phosphor::regular::TEXT_H_TWO
                                            ))
                                            .clicked()
                                        {
                                            self.apply_heading(&ctx, 2);
                                            ui.close();
                                        }
                                        if ui
                                            .button(format!(
                                                "{} Heading 3",
                                                egui_phosphor::regular::TEXT_H_THREE
                                            ))
                                            .clicked()
                                        {
                                            self.apply_heading(&ctx, 3);
                                            ui.close();
                                        }
                                    },
                                );
                                ui.menu_button(
                                    format!("{} Lists", egui_phosphor::regular::LIST_BULLETS),
                                    |ui| {
                                        if ui
                                            .button(format!(
                                                "{} Bulleted list",
                                                egui_phosphor::regular::LIST_BULLETS
                                            ))
                                            .clicked()
                                        {
                                            self.apply_formatting(&ctx, Action::ToggleBulletList);
                                            ui.close();
                                        }
                                        if ui
                                            .button(format!(
                                                "{} Numbered list",
                                                egui_phosphor::regular::LIST_NUMBERS
                                            ))
                                            .clicked()
                                        {
                                            self.apply_formatting(&ctx, Action::ToggleNumberedList);
                                            ui.close();
                                        }
                                    },
                                );
                                ui.separator();
                                if ui
                                    .button(format!(
                                        "{} Bold (Ctrl+B)",
                                        egui_phosphor::regular::TEXT_B
                                    ))
                                    .clicked()
                                {
                                    self.apply_formatting(&ctx, Action::ToggleBold);
                                    ui.close();
                                }
                                if ui
                                    .button(format!(
                                        "{} Italic (Ctrl+I)",
                                        egui_phosphor::regular::TEXT_ITALIC
                                    ))
                                    .clicked()
                                {
                                    self.apply_formatting(&ctx, Action::ToggleItalic);
                                    ui.close();
                                }
                                if ui
                                    .button(format!(
                                        "{} Strikethrough (Ctrl+Shift+X)",
                                        egui_phosphor::regular::TEXT_STRIKETHROUGH
                                    ))
                                    .clicked()
                                {
                                    self.apply_formatting(&ctx, Action::ToggleStrikethrough);
                                    ui.close();
                                }
                                ui.separator();
                                if ui
                                    .button(format!(
                                        "{} Link… (Ctrl+K)",
                                        egui_phosphor::regular::LINK
                                    ))
                                    .clicked()
                                {
                                    self.apply_formatting(&ctx, Action::InsertLink);
                                    ui.close();
                                }
                                if ui
                                    .button(format!("{} Table", egui_phosphor::regular::TABLE))
                                    .clicked()
                                {
                                    self.apply_table_insert(&ctx);
                                    ui.close();
                                }
                                ui.separator();
                                if ui
                                    .button(format!(
                                        "{} Clear formatting",
                                        egui_phosphor::regular::ERASER
                                    ))
                                    .clicked()
                                {
                                    self.apply_clear_formatting(&ctx);
                                    ui.close();
                                }
                            });
                        } else {
                            ui.menu_button(
                                format!("{} H1", egui_phosphor::regular::TEXT_H),
                                |ui| {
                                    if ui
                                        .button(format!(
                                            "{} Heading 1",
                                            egui_phosphor::regular::TEXT_H_ONE
                                        ))
                                        .clicked()
                                    {
                                        self.apply_heading(&ctx, 1);
                                        ui.close();
                                    }
                                    if ui
                                        .button(format!(
                                            "{} Heading 2",
                                            egui_phosphor::regular::TEXT_H_TWO
                                        ))
                                        .clicked()
                                    {
                                        self.apply_heading(&ctx, 2);
                                        ui.close();
                                    }
                                    if ui
                                        .button(format!(
                                            "{} Heading 3",
                                            egui_phosphor::regular::TEXT_H_THREE
                                        ))
                                        .clicked()
                                    {
                                        self.apply_heading(&ctx, 3);
                                        ui.close();
                                    }
                                },
                            );
                            ui.menu_button(egui_phosphor::regular::LIST_BULLETS, |ui| {
                                if ui
                                    .button(format!(
                                        "{} Bulleted list",
                                        egui_phosphor::regular::LIST_BULLETS
                                    ))
                                    .clicked()
                                {
                                    self.apply_formatting(&ctx, Action::ToggleBulletList);
                                    ui.close();
                                }
                                if ui
                                    .button(format!(
                                        "{} Numbered list",
                                        egui_phosphor::regular::LIST_NUMBERS
                                    ))
                                    .clicked()
                                {
                                    self.apply_formatting(&ctx, Action::ToggleNumberedList);
                                    ui.close();
                                }
                            });
                            ui.separator();
                            if ui
                                .button(egui_phosphor::regular::TEXT_B)
                                .on_hover_text("Bold (Ctrl+B)")
                                .clicked()
                            {
                                self.apply_formatting(&ctx, Action::ToggleBold);
                            }
                            if ui
                                .button(egui_phosphor::regular::TEXT_ITALIC)
                                .on_hover_text("Italic (Ctrl+I)")
                                .clicked()
                            {
                                self.apply_formatting(&ctx, Action::ToggleItalic);
                            }
                            if ui
                                .button(egui_phosphor::regular::TEXT_STRIKETHROUGH)
                                .on_hover_text("Strikethrough (Ctrl+Shift+X)")
                                .clicked()
                            {
                                self.apply_formatting(&ctx, Action::ToggleStrikethrough);
                            }
                            ui.separator();
                            if ui
                                .button(egui_phosphor::regular::LINK)
                                .on_hover_text("Link (Ctrl+K)")
                                .clicked()
                            {
                                self.apply_formatting(&ctx, Action::InsertLink);
                            }
                            if ui
                                .button(egui_phosphor::regular::TABLE)
                                .on_hover_text("Table")
                                .clicked()
                            {
                                self.apply_table_insert(&ctx);
                            }
                            ui.separator();
                            if ui
                                .button(egui_phosphor::regular::ERASER)
                                .on_hover_text("Clear formatting")
                                .clicked()
                            {
                                self.apply_clear_formatting(&ctx);
                            }
                        }
                    }

                    // Region 3: MD/TXT Switcher (right-aligned)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().button_padding = egui::vec2(10.0, 4.0);
                        if let Some(document_id) = self.session.active_tab {
                            if let Some(document) = self.workspace.document(document_id) {
                                let mut requested_kind = document.kind;
                                ui.selectable_value(&mut requested_kind, DocKind::Txt, "TXT");
                                ui.selectable_value(&mut requested_kind, DocKind::Md, "MD");
                                if requested_kind != document.kind {
                                    self.flush_active_now();
                                    if let Err(error) = self
                                        .workspace
                                        .set_document_kind(document_id, requested_kind)
                                    {
                                        self.report_error(format!(
                                            "Could not change document type: {error}"
                                        ));
                                    }
                                }
                            }
                        }
                    });
                });
            });

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
                            .hint_text(format!(
                                "{} Search notes…",
                                egui_phosphor::regular::MAGNIFYING_GLASS
                            ))
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
                                    ui.label(if *is_open {
                                        egui_phosphor::regular::CHECK
                                    } else {
                                        " "
                                    })
                                    .on_hover_text(
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
                                        .small_button(egui_phosphor::regular::TRASH)
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
                    formatting::cursor_position(&document.content, self.cursor_offset),
                    document.content.chars().count(),
                    formatting::line_ending_label(&document.content),
                )
            })
        });
        egui::Panel::bottom("footer")
            .exact_size(38.0)
            .frame(
                egui::Frame::new()
                    .fill(ui.style().visuals.panel_fill)
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 4,
                        bottom: 4,
                    }),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(12.0, 0.0);
                ui.spacing_mut().button_padding = egui::vec2(6.0, 3.0);
                ui.horizontal(|ui| {
                    ui.set_height(24.0);
                    if let Some(((line, column), character_count, line_ending)) = editor_stats {
                        ui.label(format!("Ln {line}, Col {column}"));
                        ui.separator();
                        ui.label(format!("{character_count} chars"));
                        ui.separator();
                        ui.label(if active_is_markdown {
                            "Markdown"
                        } else {
                            "Plain text"
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label("UTF-8");
                            ui.separator();
                            ui.label(line_ending);
                            ui.separator();
                            if ui
                                .button(egui_phosphor::regular::MAGNIFYING_GLASS_PLUS)
                                .on_hover_text("Zoom in")
                                .clicked()
                            {
                                self.zoom = (self.zoom + 0.1).min(3.0);
                            }
                            if ui
                                .button(format!("{:.0}%", self.zoom * 100.0))
                                .on_hover_text("Reset zoom")
                                .clicked()
                            {
                                self.zoom = 1.0;
                            }
                            if ui
                                .button(egui_phosphor::regular::MAGNIFYING_GLASS_MINUS)
                                .on_hover_text("Zoom out")
                                .clicked()
                            {
                                self.zoom = (self.zoom - 0.1).max(0.5);
                            }
                        });
                    } else {
                        ui.label("No tab open");
                    }
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(ui.style().visuals.panel_fill)
                    .inner_margin(egui::Margin {
                        left: 24,
                        right: 24,
                        top: 16,
                        bottom: 20,
                    }),
            )
            .show(ui, |ui| {
                let Some(document_id) = self.session.active_tab else {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            "No tabs open — create a new note, or pick one from the tabs list.",
                        );
                    });
                    return;
                };
                if !self.workspace.set_active_by_id(document_id) {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            "No tabs open — create a new note, or pick one from the tabs list.",
                        );
                    });
                    return;
                }
                let is_markdown = self.workspace.active_document().kind == DocKind::Md;
                let zoom = self.zoom;
                let font_family = self.theme_draft.content_font_family();
                let text_color = if ui.visuals().dark_mode {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::BLACK
                };
                let editor_id = self.editor_id();
                let mut layouter =
                    move |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, wrap_width: f32| {
                        let mut job = if is_markdown {
                            highlighting::highlight(buffer.as_str(), zoom, &font_family, text_color)
                        } else {
                            highlighting::plain(buffer.as_str(), zoom, &font_family, text_color)
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
                            .frame(egui::Frame::NONE)
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
                .default_width(480.0)
                .show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        for tab in SettingsTab::ALL {
                            if ui
                                .selectable_label(self.settings_tab == *tab, tab.title())
                                .clicked()
                            {
                                self.settings_tab = *tab;
                            }
                        }
                    });
                    ui.separator();

                    match self.settings_tab {
                        SettingsTab::Themes => {
                            self.render_themes_settings(ui, &ctx);
                        }
                        SettingsTab::Keyboard => {
                            self.render_keyboard_settings(ui);
                        }
                    }
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
