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
                .default_size(egui::vec2(560.0, 640.0))
                .min_size(egui::vec2(500.0, 480.0))
                .show(ctx, |ui| {
                    let search_response = ui
                        .horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let button_width = 28.0;
                            let search_width =
                                (ui.available_width() - button_width - 6.0).max(120.0);
                            let response = ui.add_sized(
                                [search_width, 28.0],
                                egui::TextEdit::singleline(&mut self.tabs_list_search).hint_text(
                                    format!(
                                        "{} Search notes…",
                                        egui_phosphor::regular::MAGNIFYING_GLASS
                                    ),
                                ),
                            );
                            if ui
                                .add_sized(
                                    [button_width, 28.0],
                                    egui::Button::new(egui_phosphor::regular::FOLDER_PLUS),
                                )
                                .on_hover_text("Create a folder at the top level")
                                .clicked()
                            {
                                self.begin_folder_create(None);
                            }
                            response
                        })
                        .inner;
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

                    self.show_folder_editor(ui);
                    ui.add_space(6.0);

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
                        .max_height(560.0)
                        .show(ui, |ui| {
                            let mut actions = TreeActions::default();

                            ui.label(egui::RichText::new("Opened tabs").strong());
                            ui.add_space(4.0);
                            self.show_opened_tabs(ui, &query, &mut actions);

                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("All tabs").strong());
                            ui.add_space(4.0);
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
                    self.save_session();
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

    fn show_opened_tabs(&mut self, ui: &mut egui::Ui, query: &str, actions: &mut TreeActions) {
        let open_tabs = self.session.open_tabs.clone();
        for id in open_tabs {
            let Some(document) = self.workspace.document(id) else {
                continue;
            };
            if !query.is_empty() && !document.title.to_lowercase().contains(query) {
                continue;
            }

            let title = document.title.clone();
            let kind = document.kind;
            let selected = self.tabs_list_selected == Some(id);
            let mut row_clicked = false;
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 28.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui
                        .add_sized(
                            [28.0, 24.0],
                            egui::Button::new(egui_phosphor::regular::TRASH),
                        )
                        .on_hover_text("Delete note permanently")
                        .clicked()
                    {
                        actions.delete_note = Some(id);
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        row_clicked = ui
                            .selectable_label(
                                selected,
                                format!(
                                    "{}  {}",
                                    title,
                                    if kind == DocKind::Md { "[MD]" } else { "[TXT]" }
                                ),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked();
                    });
                },
            );
            if row_clicked {
                self.tabs_list_selected = Some(id);
                actions.open_note = Some(id);
            }
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
                    let mut expand_for_create = false;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.add_space(depth as f32 * 18.0);

                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 28.0),
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .add_sized(
                                        [28.0, 24.0],
                                        egui::Button::new(egui_phosphor::regular::TRASH),
                                    )
                                    .on_hover_text("Delete folder and move its contents up")
                                    .clicked()
                                {
                                    actions.delete_folder = Some(id);
                                }
                                if ui
                                    .add_sized(
                                        [28.0, 24.0],
                                        egui::Button::new(egui_phosphor::regular::PENCIL),
                                    )
                                    .on_hover_text("Rename folder")
                                    .clicked()
                                {
                                    self.begin_folder_rename(id, &title);
                                }
                                if ui
                                    .add_sized(
                                        [28.0, 24.0],
                                        egui::Button::new(egui_phosphor::regular::PLUS),
                                    )
                                    .on_hover_text("Create a subfolder")
                                    .clicked()
                                {
                                    self.begin_folder_create(Some(id));
                                    expand_for_create = true;
                                }
                                ui.with_layout(
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        let row = ui
                                            .selectable_label(
                                                false,
                                                format!(
                                                    "{}  {}",
                                                    egui_phosphor::regular::FOLDER,
                                                    title
                                                ),
                                            )
                                            .interact(egui::Sense::click_and_drag())
                                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                                        row_rect = Some(row.rect);
                                        row_clicked = row.clicked();
                                        row_dragged =
                                            row.drag_started_by(egui::PointerButton::Primary);
                                        drop_placement = self.drop_placement_for(row, item);
                                    },
                                );
                            },
                        );
                    });
                    if expand_for_create {
                        self.expanded_folders.insert(id);
                        self.save_session();
                    }
                    if row_clicked {
                        if expanded {
                            self.expanded_folders.remove(&id);
                        } else {
                            self.expanded_folders.insert(id);
                        }
                        self.save_session();
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
                    let selected = self.tabs_list_selected == Some(id);
                    let mut row_clicked = false;
                    let mut row_dragged = false;
                    let mut row_rect = None;
                    let mut drop_placement = None;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let document_indent =
                            depth as f32 * 18.0 + if depth > 0 { 28.0 } else { 0.0 };
                        ui.add_space(document_indent);
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 28.0),
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .add_sized(
                                        [28.0, 24.0],
                                        egui::Button::new(egui_phosphor::regular::TRASH),
                                    )
                                    .on_hover_text("Delete note permanently")
                                    .clicked()
                                {
                                    actions.delete_note = Some(id);
                                }
                                ui.with_layout(
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        let row = ui
                                            .selectable_label(
                                                selected,
                                                format!(
                                                    "{}  {}",
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
                                        row_dragged =
                                            row.drag_started_by(egui::PointerButton::Primary);
                                        drop_placement = self.drop_placement_for(row, item);
                                    },
                                );
                            },
                        );
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
                    self.save_session();
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
