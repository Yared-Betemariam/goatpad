use super::*;

impl GoatpadApp {
    pub(super) fn render_themes_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
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
                ui.add(egui::TextEdit::singleline(&mut draft.name).desired_width(240.0));
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
            ui.label(format!(
                "{} installed font{} available (Goatpad checks this computer at startup)",
                self.font_options.len(),
                if self.font_options.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ));
            egui::ComboBox::from_label("System font")
                .selected_text(&draft.system_font)
                .show_ui(ui, |ui| {
                    for font in &self.font_options {
                        changed |= ui
                            .selectable_value(&mut draft.system_font, font.clone(), font)
                            .changed();
                    }
                });

            egui::ComboBox::from_label("Content font")
                .selected_text(&draft.content_font)
                .show_ui(ui, |ui| {
                    for font in &self.font_options {
                        changed |= ui
                            .selectable_value(&mut draft.content_font, font.clone(), font)
                            .changed();
                    }
                });

            changed |= ui
                .add(
                    egui::Slider::new(
                        &mut draft.font_size,
                        appearance::MIN_FONT_SIZE..=appearance::MAX_FONT_SIZE,
                    )
                    .text("Font size"),
                )
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
}
