use super::super::*;
use super::layout::FIND_BAR_HEIGHT;

impl GoatpadApp {
    pub(super) fn show_find_bar_panel(&mut self, ui: &mut egui::Ui) {
        if self.find.is_none() {
            return;
        }

        let ctx = ui.ctx().clone();
        let find_response = egui::Panel::top("find_bar")
            .exact_size(FIND_BAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(ui.style().visuals.panel_fill)
                    .stroke(egui::Stroke::NONE)
                    .inner_margin(egui::Margin::symmetric(10, 0)),
            )
            .show(ui, |ui| {
                let matches = self.current_find_matches();
                let match_count = matches.len();
                let (scope, current) = self
                    .find
                    .as_ref()
                    .map(|find| (find.scope, find.current))
                    .expect("find bar requires find state");
                let mut close = false;
                let mut navigate = None;

                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_height(FIND_BAR_HEIGHT);
                    close = ui
                        .add(
                            egui::Button::new(egui_phosphor::regular::X)
                                .frame_when_inactive(false)
                                .min_size(egui::vec2(24.0, 24.0)),
                        )
                        .on_hover_text("Close find")
                        .clicked();
                    let (scope_icon, scope_tooltip) = match scope {
                        FindScope::ActiveNote => {
                            (egui_phosphor::regular::NOTE, "Finding in current note")
                        }
                        FindScope::AllNotes => {
                            (egui_phosphor::regular::FILES, "Finding in all notes")
                        }
                    };
                    ui.label(egui::RichText::new(scope_icon).size(18.0))
                        .on_hover_text(scope_tooltip);

                    let input_width = (ui.available_width() - 180.0).max(80.0);
                    let find = self.find.as_mut().expect("find bar requires find state");
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut find.query)
                            .desired_width(input_width)
                            .hint_text("Find…"),
                    );
                    if find.focus_input {
                        response.request_focus();
                        find.focus_input = false;
                    }
                    if response.changed() {
                        find.current = None;
                    }

                    let position = current
                        .filter(|index| *index < match_count)
                        .map_or(0, |index| index + 1);
                    ui.label(format!("{position} of {match_count}"));
                    if ui
                        .add_enabled(
                            match_count > 0,
                            egui::Button::new(egui_phosphor::regular::ARROW_UP)
                                .frame_when_inactive(false),
                        )
                        .on_hover_text("Previous match (Shift+Enter)")
                        .clicked()
                    {
                        navigate = Some(false);
                    }
                    if ui
                        .add_enabled(
                            match_count > 0,
                            egui::Button::new(egui_phosphor::regular::ARROW_DOWN)
                                .frame_when_inactive(false),
                        )
                        .on_hover_text("Next match (Enter)")
                        .clicked()
                    {
                        navigate = Some(true);
                    }
                });

                if close {
                    self.close_find(&ctx);
                } else if let Some(forward) = navigate {
                    self.navigate_find(&ctx, forward);
                }
            });
        ui.painter().hline(
            find_response.response.rect.left()..=find_response.response.rect.right(),
            find_response.response.rect.top(),
            egui::Stroke::new(1.75, self.theme_draft.border_color()),
        );
    }
}
