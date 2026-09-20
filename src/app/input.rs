use super::*;

impl GoatpadApp {
    pub(in crate::app) fn set_tabs_list_open(&mut self, open: bool) {
        self.tabs_list_open = open;
        self.focus_tabs_list_search = open;
        self.tabs_list_selected = None;
        if !open {
            self.tabs_list_search.clear();
            self.dragged_workspace_item = None;
            self.workspace_drop_target = None;
            self.folder_editor = None;
        }
    }

    pub(in crate::app) fn toggle_tabs_list(&mut self) {
        self.set_tabs_list_open(!self.tabs_list_open);
    }

    /// Closes every transient panel and popup when Escape is pressed.
    ///
    /// Keep this handling in one place so panels added later do not need to
    /// implement their own, potentially conflicting, Escape behavior.
    fn close_escape_targets(&mut self, ctx: &egui::Context) -> bool {
        let has_open_target = self.find.is_some()
            || self.tabs_list_open
            || self.settings_open
            || self.delete_confirmation.is_some()
            || self.theme_delete_confirm.is_some()
            || self.rebinding.is_some()
            || self.renaming_document.is_some()
            || self.spellcheck_menu.is_some()
            || !self.multi_cursor_offsets.is_empty()
            || egui::Popup::is_any_open(ctx);
        if !has_open_target
            || !ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            return false;
        }

        egui::Popup::close_all(ctx);

        if self.find.is_some() {
            self.close_find(ctx);
        }
        self.set_tabs_list_open(false);
        self.settings_open = false;
        self.delete_confirmation = None;
        self.theme_delete_confirm = None;
        self.rebinding = None;
        self.spellcheck_menu = None;
        self.clear_multi_cursors();
        if self.renaming_document.is_some() {
            self.cancel_rename();
        }
        true
    }

    pub(in crate::app) fn dispatch_hotkeys(&mut self, ctx: &egui::Context) {
        if self.close_escape_targets(ctx) {
            return;
        }
        if let Some(action) = self.rebinding {
            if let Some(binding) =
                ctx.input(|input| input.events.iter().find_map(hotkeys::keybinding_from_event))
            {
                self.settings.keybindings.insert(action, binding);
                if let Err(error) = self.settings.save(&self.paths) {
                    self.report_error(format!("Could not save keyboard shortcuts: {error}"));
                }
                self.rebinding = None;
            }
            return;
        }
        if self.find.is_some() {
            let previous = ctx.input_mut(|input| {
                input.consume_key(
                    egui::Modifiers {
                        shift: true,
                        ..egui::Modifiers::NONE
                    },
                    egui::Key::Enter,
                )
            });
            if previous {
                self.navigate_find(ctx, false);
                return;
            }
            let next =
                ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            if next {
                self.navigate_find(ctx, true);
                return;
            }
        }
        let action = Action::ALL.into_iter().find(|action| {
            self.settings
                .keybindings
                .get(action)
                .is_some_and(|binding| {
                    ctx.input_mut(|input| input.consume_key(binding.modifiers, binding.key))
                })
        });
        let Some(action) = action else {
            return;
        };
        match action {
            Action::NewTab => self.create_tab(),
            Action::CloseTab => {
                if let Some(id) = self.session.active_tab {
                    self.close_tab(id);
                }
            }
            Action::NextTab => {
                let previous = self.session.active_tab;
                if let Some(id) = self.session.cycle_tab(true) {
                    self.session.active_tab = previous;
                    self.activate_tab(id);
                }
            }
            Action::PreviousTab => {
                let previous = self.session.active_tab;
                if let Some(id) = self.session.cycle_tab(false) {
                    self.session.active_tab = previous;
                    self.activate_tab(id);
                }
            }
            Action::FindInNote => self.open_find(FindScope::ActiveNote),
            Action::FindInAllNotes => self.open_find(FindScope::AllNotes),
            Action::OpenTabsList => self.toggle_tabs_list(),
            Action::ToggleDocumentKind => self.toggle_active_document_kind(),
            Action::ToggleMarkdownPreview => self.toggle_active_markdown_preview(),
            Action::OpenSettings => self.settings_open = !self.settings_open,
            action
                if action.is_formatting()
                    && self
                        .session
                        .active_tab
                        .and_then(|id| self.workspace.document(id))
                        .is_some_and(|document| document.kind == DocKind::Md) =>
            {
                self.apply_formatting(ctx, action);
            }
            _ => {}
        }
    }
}
