use super::super::*;

impl GoatpadApp {
    pub(super) fn show_tabs_list(&mut self, ctx: &egui::Context) {
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
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .collapsible(false)
                .resizable(true)
                .default_width(360.0)
                .show(ctx, |ui| {
                    let search_response = ui.add(
                        egui::TextEdit::singleline(&mut self.tabs_list_search)
                            .hint_text(format!(
                                "{} Search notes…",
                                egui_phosphor::regular::MAGNIFYING_GLASS
                            ))
                            .desired_width(f32::INFINITY),
                    );
                    if self.focus_tabs_list_search {
                        search_response.request_focus();
                        self.focus_tabs_list_search = false;
                    }
                    ui.separator();
                    let query = self.tabs_list_search.trim().to_lowercase();
                    let visible_note_ids = notes
                        .iter()
                        .filter(|(_, title, _, _)| {
                            query.is_empty() || title.to_lowercase().contains(&query)
                        })
                        .map(|(id, _, _, _)| *id)
                        .collect::<Vec<_>>();

                    if search_response.changed() {
                        self.tabs_list_selected = visible_note_ids.first().copied();
                    } else if self.tabs_list_selected.is_none()
                        || !visible_note_ids.contains(&self.tabs_list_selected.unwrap())
                    {
                        self.tabs_list_selected = visible_note_ids.first().copied();
                    }

                    let mut selected_index = self.tabs_list_selected.and_then(|id| {
                        visible_note_ids
                            .iter()
                            .position(|visible_id| *visible_id == id)
                    });
                    if !visible_note_ids.is_empty() {
                        let moved_down = ui.input_mut(|input| {
                            input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
                        });
                        let moved_up = if moved_down {
                            false
                        } else {
                            ui.input_mut(|input| {
                                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
                            })
                        };
                        if moved_down {
                            selected_index = Some(
                                selected_index
                                    .map_or(0, |index| (index + 1) % visible_note_ids.len()),
                            );
                        } else if moved_up {
                            selected_index =
                                Some(selected_index.map_or(visible_note_ids.len() - 1, |index| {
                                    if index == 0 {
                                        visible_note_ids.len() - 1
                                    } else {
                                        index - 1
                                    }
                                }));
                        }
                    }
                    self.tabs_list_selected = selected_index.map(|index| visible_note_ids[index]);
                    if ui.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    }) {
                        requested_list_open = self.tabs_list_selected;
                    }

                    egui::ScrollArea::vertical()
                        .max_height(420.0)
                        .show(ui, |ui| {
                            for (id, title, _, is_open) in &notes {
                                if !visible_note_ids.contains(id) {
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
                                    let selected = self.tabs_list_selected == Some(*id);
                                    if ui
                                        .selectable_label(selected, title)
                                        .on_hover_text("Open note")
                                        .clicked()
                                    {
                                        self.tabs_list_selected = Some(*id);
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
                            if visible_note_ids.is_empty() {
                                ui.label(if notes.is_empty() {
                                    "No notes saved."
                                } else {
                                    "No matching notes."
                                });
                            }
                        });
                });
            self.tabs_list_open = list_open;
            if !self.tabs_list_open {
                self.focus_tabs_list_search = false;
                self.tabs_list_selected = None;
            }
        }
        if let Some(id) = requested_list_open {
            self.tabs_list_open = false;
            self.activate_tab(id);
        }
        if let Some(id) = requested_list_delete {
            self.delete_confirmation = Some(id);
        }
    }
}
