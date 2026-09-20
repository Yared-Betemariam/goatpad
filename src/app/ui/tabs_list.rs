use super::super::*;
use crate::domain::workspace::{DropPlacement, WorkspaceItem};

#[derive(Default)]
struct TreeActions {
    open_note: Option<Uuid>,
    delete_note: Option<Uuid>,
    delete_folder: Option<Uuid>,
}

impl GoatpadApp {
    pub(super) fn show_tabs_list(&mut self, ctx: &egui::Context) {
        let mut requested_list_open = None;
        let mut requested_list_delete = None;
        let mut requested_folder_delete = None;
        let mut close_list = false;

        if self.tabs_list_open {
            let mut list_open = true;
            let query = self.tabs_list_search.trim().to_lowercase();
            let visible_note_ids = self.visible_note_ids(&query);
            let mut selected_index = self.tabs_list_selected.and_then(|id| {
                visible_note_ids
                    .iter()
                    .position(|visible_id| *visible_id == id)
            });

            egui::Window::new("Tabs list")
                .open(&mut list_open)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .collapsible(false)
                .resizable(true)
                .default_width(420.0)
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
                    if search_response.changed() {
                        self.tabs_list_selected = visible_note_ids.first().copied();
                        selected_index = self.tabs_list_selected.and_then(|id| {
                            visible_note_ids
                                .iter()
                                .position(|visible_id| *visible_id == id)
                        });
                    } else if self.tabs_list_selected.is_none()
                        || !visible_note_ids.contains(&self.tabs_list_selected.unwrap())
                    {
                        self.tabs_list_selected = visible_note_ids.first().copied();
                        selected_index = self.tabs_list_selected.and_then(|id| {
                            visible_note_ids
                                .iter()
                                .position(|visible_id| *visible_id == id)
                        });
                    }

                    ui.horizontal(|ui| {
                        if ui
                            .button(format!(
                                "{} New folder",
                                egui_phosphor::regular::FOLDER_PLUS
                            ))
                            .on_hover_text("Create a folder at the top level")
                            .clicked()
                        {
                            self.begin_folder_create(None);
                        }
                        ui.label("Drag notes or folders to organize them");
                    });
                    self.show_folder_editor(ui);
                    ui.separator();

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

                    self.workspace_drop_target = None;
                    egui::ScrollArea::vertical()
                        .max_height(460.0)
                        .show(ui, |ui| {
                            let mut actions = TreeActions::default();
                            self.show_workspace_tree(
                                ui,
                                None,
                                0,
                                &query,
                                &visible_note_ids,
                                &mut actions,
                            );
                            requested_list_open = requested_list_open.or(actions.open_note);
                            requested_list_delete = actions.delete_note;
                            requested_folder_delete = actions.delete_folder;
                            if self.workspace.documents.is_empty()
                                && self.workspace.folders.is_empty()
                            {
                                ui.label("No notes or folders saved.");
                            } else if visible_note_ids.is_empty() {
                                ui.label("No matching notes.");
                            }
                        });

                    if self.dragged_workspace_item.is_some()
                        && ui.input(|input| input.pointer.any_released())
                    {
                        if let (Some(item), Some((target, placement))) = (
                            self.dragged_workspace_item.take(),
                            self.workspace_drop_target.take(),
                        ) {
                            match self.workspace.move_item(item, target, placement) {
                                Ok(true) => self.report_success("Item moved"),
                                Ok(false) => {}
                                Err(error) => {
                                    self.report_error(format!("Could not move item: {error}"));
                                }
                            }
                        } else {
                            self.dragged_workspace_item = None;
                        }
                    }
                });
            self.tabs_list_open = list_open;
            close_list = !list_open;
        }

        if close_list {
            self.set_tabs_list_open(false);
        }
        if let Some(id) = requested_list_open {
            self.set_tabs_list_open(false);
            self.activate_tab(id);
        }
        if let Some(id) = requested_list_delete {
            self.delete_confirmation = Some(id);
        }
        if let Some(id) = requested_folder_delete {
            match self.workspace.delete_folder(id) {
                Ok(true) => {
                    self.expanded_folders.remove(&id);
                    self.report_success("Folder deleted; its contents were moved up");
                }
                Ok(false) => {}
                Err(error) => self.report_error(format!("Could not delete folder: {error}")),
            }
        }
    }

    fn visible_note_ids(&self, query: &str) -> Vec<Uuid> {
        let mut ids = Vec::new();
        self.collect_visible_note_ids(None, query, &mut ids);
        ids
    }

    fn collect_visible_note_ids(&self, parent_id: Option<Uuid>, query: &str, ids: &mut Vec<Uuid>) {
        for item in self.workspace.children_of(parent_id) {
            match item {
                WorkspaceItem::Document(id) => {
                    if query.is_empty()
                        || self
                            .workspace
                            .document(id)
                            .is_some_and(|document| document.title.to_lowercase().contains(query))
                    {
                        ids.push(id);
                    }
                }
                WorkspaceItem::Folder(id) => {
                    if query.is_empty() && !self.expanded_folders.contains(&id) {
                        continue;
                    }
                    self.collect_visible_note_ids(Some(id), query, ids);
                }
            }
        }
    }

    fn item_matches_query(&self, item: WorkspaceItem, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        match item {
            WorkspaceItem::Document(id) => self
                .workspace
                .document(id)
                .is_some_and(|document| document.title.to_lowercase().contains(query)),
            WorkspaceItem::Folder(id) => self
                .workspace
                .children_of(Some(id))
                .into_iter()
                .any(|child| self.item_matches_query(child, query)),
        }
    }

    fn show_workspace_tree(
        &mut self,
        ui: &mut egui::Ui,
        parent_id: Option<Uuid>,
        depth: usize,
        query: &str,
        visible_note_ids: &[Uuid],
        actions: &mut TreeActions,
    ) {
        let children = self.workspace.children_of(parent_id);
        for item in children {
            if !self.item_matches_query(item, query) {
                continue;
            }
            match item {
                WorkspaceItem::Folder(id) => {
                    let Some(folder) = self.workspace.folder(id) else {
                        continue;
                    };
                    let title = folder.name.clone();
                    let expanded = query.is_empty() && self.expanded_folders.contains(&id);
                    let mut row_clicked = false;
                    let mut row_dragged = false;
                    let mut row_rect = None;
                    let mut drop_placement = None;
                    ui.horizontal(|ui| {
                        ui.add_space(depth as f32 * 18.0);
                        let arrow = if expanded || !query.is_empty() {
                            "▾"
                        } else {
                            "▸"
                        };
                        if ui
                            .small_button(arrow)
                            .on_hover_text(if expanded {
                                "Collapse folder"
                            } else {
                                "Expand folder"
                            })
                            .clicked()
                        {
                            if expanded {
                                self.expanded_folders.remove(&id);
                            } else {
                                self.expanded_folders.insert(id);
                            }
                        }
                        let row = ui
                            .selectable_label(
                                false,
                                format!("{}  {}", egui_phosphor::regular::FOLDER, title),
                            )
                            .interact(egui::Sense::click_and_drag())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        row_rect = Some(row.rect);
                        row_clicked = row.clicked();
                        row_dragged = row.drag_started_by(egui::PointerButton::Primary);
                        drop_placement = self.drop_placement_for(row, item);
                        if ui
                            .small_button("+")
                            .on_hover_text("Create a subfolder")
                            .clicked()
                        {
                            self.begin_folder_create(Some(id));
                            self.expanded_folders.insert(id);
                        }
                        if ui
                            .small_button(egui_phosphor::regular::PENCIL)
                            .on_hover_text("Rename folder")
                            .clicked()
                        {
                            self.begin_folder_rename(id, &title);
                        }
                        if ui
                            .small_button(egui_phosphor::regular::TRASH)
                            .on_hover_text("Delete folder and move its contents up")
                            .clicked()
                        {
                            actions.delete_folder = Some(id);
                        }
                    });
                    if row_clicked {
                        if expanded {
                            self.expanded_folders.remove(&id);
                        } else {
                            self.expanded_folders.insert(id);
                        }
                    }
                    if row_dragged {
                        self.dragged_workspace_item = Some(item);
                    }
                    if self.dragged_workspace_item.is_some() && drop_placement.is_some() {
                        self.workspace_drop_target = drop_placement;
                    }
                    if self.workspace_drop_target == drop_placement && drop_placement.is_some() {
                        if let Some(rect) = row_rect {
                            ui.painter().rect_stroke(
                                rect,
                                4.0,
                                egui::Stroke::new(1.5, self.theme_draft.primary.0),
                                egui::StrokeKind::Outside,
                            );
                        }
                    }
                    if expanded || !query.is_empty() {
                        self.show_workspace_tree(
                            ui,
                            Some(id),
                            depth + 1,
                            query,
                            visible_note_ids,
                            actions,
                        );
                    }
                }
                WorkspaceItem::Document(id) => {
                    let Some(document) = self.workspace.document(id) else {
                        continue;
                    };
                    let title = document.title.clone();
                    let is_open = self.session.open_tabs.contains(&id);
                    let selected = self.tabs_list_selected == Some(id);
                    let mut row_clicked = false;
                    let mut row_dragged = false;
                    let mut row_rect = None;
                    let mut drop_placement = None;
                    ui.horizontal(|ui| {
                        ui.add_space(depth as f32 * 18.0 + 22.0);
                        let row = ui
                            .selectable_label(
                                selected,
                                format!(
                                    "{} {} {}",
                                    if is_open { "●" } else { "○" },
                                    title,
                                    if document.kind == DocKind::Md {
                                        "[MD]"
                                    } else {
                                        "[TXT]"
                                    }
                                ),
                            )
                            .interact(egui::Sense::click_and_drag())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        row_rect = Some(row.rect);
                        row_clicked = row.clicked();
                        row_dragged = row.drag_started_by(egui::PointerButton::Primary);
                        drop_placement = self.drop_placement_for(row, item);
                        if ui
                            .small_button(egui_phosphor::regular::TRASH)
                            .on_hover_text("Delete note permanently")
                            .clicked()
                        {
                            actions.delete_note = Some(id);
                        }
                    });
                    if row_clicked {
                        self.tabs_list_selected = Some(id);
                        actions.open_note = Some(id);
                    }
                    if row_dragged {
                        self.dragged_workspace_item = Some(item);
                    }
                    if self.dragged_workspace_item.is_some() && drop_placement.is_some() {
                        self.workspace_drop_target = drop_placement;
                    }
                    if self.workspace_drop_target == drop_placement && drop_placement.is_some() {
                        if let Some(rect) = row_rect {
                            ui.painter().rect_stroke(
                                rect,
                                4.0,
                                egui::Stroke::new(1.5, self.theme_draft.primary.0),
                                egui::StrokeKind::Outside,
                            );
                        }
                    }
                    if !visible_note_ids.contains(&id) && !query.is_empty() {
                        continue;
                    }
                }
            }
        }
    }

    fn drop_placement_for(
        &self,
        response: egui::Response,
        item: WorkspaceItem,
    ) -> Option<(WorkspaceItem, DropPlacement)> {
        let Some(dragged) = self.dragged_workspace_item else {
            return None;
        };
        if dragged == item || !response.hovered() {
            return None;
        }
        let pointer_y = response
            .ctx
            .input(|input| input.pointer.hover_pos())
            .map_or(response.rect.center().y, |position| position.y);
        let relative = (pointer_y - response.rect.top()) / response.rect.height().max(1.0);
        let placement =
            if matches!(item, WorkspaceItem::Folder(_)) && (0.25..=0.75).contains(&relative) {
                DropPlacement::Into
            } else if relative < 0.5 {
                DropPlacement::Before
            } else {
                DropPlacement::After
            };
        Some((item, placement))
    }

    fn begin_folder_create(&mut self, parent_id: Option<Uuid>) {
        self.folder_editor = Some(FolderEditor {
            id: None,
            parent_id,
            buffer: String::new(),
            focus: true,
        });
    }

    fn begin_folder_rename(&mut self, id: Uuid, name: &str) {
        self.folder_editor = Some(FolderEditor {
            id: Some(id),
            parent_id: None,
            buffer: name.to_owned(),
            focus: true,
        });
    }

    fn show_folder_editor(&mut self, ui: &mut egui::Ui) {
        let Some(editor) = self.folder_editor.as_mut() else {
            return;
        };
        let mut commit = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            ui.label(if editor.id.is_some() {
                "Rename folder:"
            } else {
                "New folder:"
            });
            let response = ui.add(
                egui::TextEdit::singleline(&mut editor.buffer)
                    .desired_width(180.0)
                    .hint_text("Folder name"),
            );
            if editor.focus {
                response.request_focus();
                editor.focus = false;
            }
            if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                commit = true;
            }
            if ui.button("Save").clicked() {
                commit = true;
            }
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
        });
        if cancel {
            self.folder_editor = None;
        } else if commit {
            let Some(editor) = self.folder_editor.take() else {
                return;
            };
            let result = if let Some(id) = editor.id {
                self.workspace.rename_folder(id, &editor.buffer).map(|_| id)
            } else {
                self.workspace
                    .create_folder(editor.parent_id, &editor.buffer)
            };
            match result {
                Ok(id) => {
                    self.expanded_folders.insert(id);
                    self.report_success(if editor.id.is_some() {
                        "Folder renamed"
                    } else {
                        "Folder created"
                    });
                }
                Err(error) => {
                    self.folder_editor = Some(FolderEditor {
                        id: editor.id,
                        parent_id: editor.parent_id,
                        buffer: editor.buffer,
                        focus: true,
                    });
                    self.report_error(format!("Could not save folder: {error}"));
                }
            }
        }
    }
}
