use super::super::*;

impl GoatpadApp {
    pub(super) fn show_confirmations(&mut self, ctx: &egui::Context) {
        if let Some(id) = self.delete_confirmation {
            let title = self
                .workspace
                .document(id)
                .map_or("Untitled", |document| document.title.as_str())
                .to_owned();
            egui::Window::new("Delete note?")
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Delete \"{title}\" permanently? This cannot be undone."
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            self.delete_confirmation = None;
                            self.delete_note(id);
                        }
                        if ui.button("Cancel").clicked() {
                            self.delete_confirmation = None;
                        }
                    });
                });
        }

        if let Some(to_delete) = self.theme_delete_confirm.clone() {
            egui::Window::new("Delete theme?")
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
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
    }
}
