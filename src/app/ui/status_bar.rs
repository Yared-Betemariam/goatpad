use super::super::*;
use super::layout::STATUS_BAR_HEIGHT;

impl GoatpadApp {
    pub(super) fn show_status_bar(&mut self, ui: &mut egui::Ui, active_is_markdown: bool) {
        let editor_stats = self.session.active_tab.and_then(|id| {
            self.workspace.document(id).map(|document| {
                (
                    formatting::cursor_position(&document.content, self.cursor_offset),
                    document.content.chars().count(),
                    formatting::line_ending_label(&document.content),
                )
            })
        });

        let footer_response = egui::Panel::bottom("footer")
            .exact_size(STATUS_BAR_HEIGHT)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(self.theme_draft.footer_color())
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 4,
                        bottom: 4,
                    }),
            )
            .show(ui, |ui| {
                let footer_text_color = ui.visuals().text_color().gamma_multiply(0.8);
                ui.visuals_mut().override_text_color = Some(footer_text_color);
                for font_id in ui.style_mut().text_styles.values_mut() {
                    font_id.size *= 0.8;
                }
                ui.spacing_mut().item_spacing = egui::vec2(12.0, 0.0);
                ui.spacing_mut().button_padding = egui::vec2(6.0, 3.0);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if let Some(((line, column), character_count, line_ending)) = editor_stats {
                        ui.scope(|ui| {
                            ui.set_height(16.0);
                            ui.label(format!("Ln {line}, Col {column}"));
                            ui.separator();
                            ui.label(format!("{character_count} characters"));
                            ui.separator();
                            ui.label(if active_is_markdown {
                                "Markdown"
                            } else {
                                "Plain text"
                            });
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.scope(|ui| {
                                ui.set_height(16.0);
                                ui.label("UTF-8");
                                ui.separator();
                                ui.label(line_ending);
                                ui.separator();
                            });

                            ui.scope(|ui| {
                                ui.set_height(24.0);
                                ui.spacing_mut().item_spacing.x -= 12.0;
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui_phosphor::regular::MAGNIFYING_GLASS_PLUS,
                                        )
                                        .frame_when_inactive(false),
                                    )
                                    .on_hover_text("Zoom in")
                                    .clicked()
                                {
                                    self.set_content_zoom(self.zoom + CONTENT_ZOOM_STEP);
                                }
                                if ui
                                    .add(
                                        egui::Button::new(format!("{:.0}%", self.zoom * 100.0))
                                            .frame_when_inactive(false),
                                    )
                                    .on_hover_text("Reset zoom")
                                    .clicked()
                                {
                                    self.set_content_zoom(DEFAULT_CONTENT_ZOOM);
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui_phosphor::regular::MAGNIFYING_GLASS_MINUS,
                                        )
                                        .frame_when_inactive(false),
                                    )
                                    .on_hover_text("Zoom out")
                                    .clicked()
                                {
                                    self.set_content_zoom(self.zoom - CONTENT_ZOOM_STEP);
                                }
                            });
                        });
                    } else {
                        ui.label("No tab open");
                    }
                });
            });

        ui.painter().hline(
            footer_response.response.rect.left()..=footer_response.response.rect.right(),
            footer_response.response.rect.top(),
            egui::Stroke::new(1.75, self.theme_draft.border_color()),
        );
    }
}
