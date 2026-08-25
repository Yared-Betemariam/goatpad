use eframe::egui;
use std::{
    sync::mpsc::Sender,
    time::{Duration, Instant},
};
use uuid::Uuid;

mod document;
mod highlighting;
mod paths;
mod persistence;
mod session;
mod workspace;

use paths::AppPaths;
use document::DocKind;
use persistence::{SaveRequest, start_writer_thread};
use session::{Session, TabState, WindowGeom};
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
}

impl GoatpadApp {
    fn new(paths: AppPaths, mut session: Session) -> std::io::Result<Self> {
        let mut workspace = Workspace::load(paths.clone())?;
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
        })
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
            });
        });
        if let Some(index) = requested_switch {
            self.switch_to(index);
        }
        if requested_new_tab {
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
                if let Err(error) = self.workspace.set_document_kind(document_id, requested_kind) {
                    eprintln!("failed to change document type: {error}");
                }
            }
            let is_markdown = self.workspace.active_document().kind == DocKind::Md;
            let mut layouter = move |ui: &egui::Ui,
                                     buffer: &dyn egui::TextBuffer,
                                     wrap_width: f32| {
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
                        .id(ui.make_persistent_id(("editor", document_id)))
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
        Box::new(move |_| Ok(Box::new(GoatpadApp::new(paths, session)?))),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::cursor_position;
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
}
