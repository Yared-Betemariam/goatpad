use eframe::egui;
use std::{sync::mpsc::Sender, time::Instant};

mod document;
mod paths;
mod persistence;
mod workspace;

use paths::AppPaths;
use persistence::{SaveRequest, start_writer_thread};
use workspace::Workspace;

struct GoatpadApp {
    workspace: Workspace,
    cursor_offset: usize,
    writer: Sender<SaveRequest>,
    last_edit: Option<Instant>,
    dirty_since: Option<Instant>,
}

impl GoatpadApp {
    fn new() -> std::io::Result<Self> {
        Ok(Self {
            workspace: Workspace::load(AppPaths::new()?)?,
            cursor_offset: 0,
            writer: start_writer_thread(),
            last_edit: None,
            dirty_since: None,
        })
    }

    fn queue_save(&mut self) {
        let id = self.workspace.document.id;
        let kind = self.workspace.document.kind;
        let path = self.workspace.document_path(id, kind);
        let document = &mut self.workspace.document;
        if !document.dirty {
            return;
        }
        let request = SaveRequest {
            id,
            kind,
            content: document.content.clone(),
            path,
        };
        if self.writer.send(request).is_ok() {
            document.dirty = false;
            self.last_edit = None;
            self.dirty_since = None;
        }
    }

    fn flush_now(&mut self) {
        if self.workspace.document.dirty {
            if let Err(error) = self.workspace.save_document(&self.workspace.document) {
                eprintln!("failed to save document on exit: {error}");
            } else {
                self.workspace.document.dirty = false;
            }
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
        let (line, column) = cursor_position(&self.workspace.document.content, self.cursor_offset);
        let character_count = self.workspace.document.content.chars().count();

        egui::Panel::bottom("footer").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Ln {line}, Col {column}"));
                ui.separator();
                ui.label(format!("{character_count} chars"));
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            let output = egui::TextEdit::multiline(&mut self.workspace.document.content)
                .min_size(ui.available_size())
                .show(ui);

            if let Some(cursor_range) = output.cursor_range {
                self.cursor_offset = cursor_range.primary.index.0;
            }
            if output.response.changed() {
                let now = Instant::now();
                self.workspace.document.dirty = true;
                self.last_edit = Some(now);
                self.dirty_since.get_or_insert(now);
            }
        });

        let now = Instant::now();
        let debounce_elapsed = self
            .last_edit
            .is_some_and(|last_edit| now.duration_since(last_edit).as_millis() >= 400);
        let hard_cap_elapsed = self
            .dirty_since
            .is_some_and(|dirty_since| now.duration_since(dirty_since).as_secs_f32() >= 2.0);
        if debounce_elapsed || hard_cap_elapsed {
            self.queue_save();
        } else if self.workspace.document.dirty {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    fn on_exit(&mut self) {
        self.flush_now();
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("Goatpad"),
        ..Default::default()
    };

    eframe::run_native(
        "Goatpad",
        options,
        Box::new(|_creation_context| Ok(Box::new(GoatpadApp::new()?))),
    )
}

#[cfg(test)]
mod tests {
    use super::cursor_position;

    #[test]
    fn cursor_starts_at_line_one_column_one() {
        assert_eq!(cursor_position("", 0), (1, 1));
        assert_eq!(cursor_position("hello", 0), (1, 1));
    }

    #[test]
    fn cursor_position_tracks_newlines() {
        let content = "first line\nsecond\nthird";

        assert_eq!(cursor_position(content, 10), (1, 11));
        assert_eq!(cursor_position(content, 11), (2, 1));
        assert_eq!(cursor_position(content, 17), (2, 7));
        assert_eq!(cursor_position(content, 18), (3, 1));
        assert_eq!(cursor_position(content, content.chars().count()), (3, 6));
    }

    #[test]
    fn cursor_position_counts_unicode_characters() {
        assert_eq!(cursor_position("café\n🦀", 4), (1, 5));
        assert_eq!(cursor_position("café\n🦀", 5), (2, 1));
        assert_eq!(cursor_position("café\n🦀", 6), (2, 2));
    }
}
