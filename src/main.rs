use eframe::egui;

mod document;

use document::Document;

#[derive(Default)]
struct GoatpadApp {
    document: Document,
    cursor_offset: usize,
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
        let (line, column) = cursor_position(&self.document.content, self.cursor_offset);
        let character_count = self.document.content.chars().count();

        egui::Panel::bottom("footer").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Ln {line}, Col {column}"));
                ui.separator();
                ui.label(format!("{character_count} chars"));
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            let output = egui::TextEdit::multiline(&mut self.document.content)
                .min_size(ui.available_size())
                .show(ui);

            if let Some(cursor_range) = output.cursor_range {
                self.cursor_offset = cursor_range.primary.index.0;
            }
        });
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
        Box::new(|_creation_context| Ok(Box::new(GoatpadApp::default()))),
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
