use super::*;

impl GoatpadApp {
    pub(super) fn render_keyboard_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Keyboard shortcuts");
        ui.label("Click a shortcut, then press its replacement key combination.");
        ui.horizontal(|ui| {
            ui.label("Restore every shortcut to its default combination.");
            if ui.button("Reset to defaults").clicked() {
                self.reset_keyboard_shortcuts();
            }
        });
        ui.add_space(8.0);
        TableBuilder::new(ui)
            .striped(true)
            .column(Column::remainder())
            .column(Column::remainder())
            .body(|mut body| {
                body.row(24.0, |mut row| {
                    row.col(|ui| {
                        ui.strong("Application");
                    });
                    row.col(|_| {});
                });
                let mut formatting_section_shown = false;
                for action in Action::ALL {
                    if action.is_formatting() && !formatting_section_shown {
                        formatting_section_shown = true;
                        body.row(24.0, |mut row| {
                            row.col(|ui| {
                                ui.strong("Markdown formatting");
                            });
                            row.col(|_| {});
                        });
                    }
                    body.row(24.0, |mut row| {
                        row.col(|ui| {
                            ui.label(action.label());
                        });
                        row.col(|ui| {
                            let text = if self.rebinding == Some(action) {
                                "Press new combo…".to_owned()
                            } else {
                                self.settings
                                    .keybindings
                                    .get(&action)
                                    .map_or_else(|| "Unbound".to_owned(), Keybinding::to_string)
                            };
                            if ui
                                .add_sized(ui.available_size(), egui::Button::new(text))
                                .clicked()
                            {
                                self.rebinding = Some(action);
                            }
                        });
                    });
                }
            });
    }
}
