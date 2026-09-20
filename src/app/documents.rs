use super::*;

impl GoatpadApp {
    pub(in crate::app) fn toggle_active_markdown_preview(&mut self) {
        let Some(document_id) = self.session.active_tab else {
            return;
        };
        if self
            .workspace
            .document(document_id)
            .is_some_and(|document| document.kind == DocKind::Md)
        {
            self.session.toggle_markdown_preview(document_id);
            self.save_session();
        }
    }

    pub(in crate::app) fn toggle_active_document_kind(&mut self) {
        self.clear_multi_cursors();
        let Some((document_id, current_kind)) = self.session.active_tab.and_then(|id| {
            self.workspace
                .document(id)
                .map(|document| (id, document.kind))
        }) else {
            return;
        };
        let requested_kind = match current_kind {
            DocKind::Md => DocKind::Txt,
            DocKind::Txt => DocKind::Md,
        };
        self.flush_active_now();
        if let Err(error) = self
            .workspace
            .set_document_kind(document_id, requested_kind)
        {
            self.report_error(format!("Could not change document type: {error}"));
        }
    }

    pub(in crate::app) fn begin_rename(&mut self, id: Uuid) {
        if let Some(document) = self
            .workspace
            .documents
            .iter()
            .find(|document| document.id == id)
        {
            self.renaming_document = Some(id);
            self.rename_buffer = document.title.clone();
            self.focus_rename = true;
        }
    }

    pub(in crate::app) fn finish_rename(&mut self) {
        let Some(id) = self.renaming_document.take() else {
            return;
        };
        match self.workspace.rename_document(id, &self.rename_buffer) {
            Ok(()) => self.workspace_index_dirty = false,
            Err(error) => {
                self.workspace_index_dirty = true;
                self.report_error(format!("Could not rename note: {error}"));
            }
        }
        self.rename_buffer.clear();
        self.focus_rename = false;
    }

    pub(in crate::app) fn cancel_rename(&mut self) {
        self.renaming_document = None;
        self.rename_buffer.clear();
        self.focus_rename = false;
    }

    pub(in crate::app) fn export_notes(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export Goatpad notes")
            .add_filter("Goatpad notes", &["json"])
            .set_file_name("goatpad-notes.json")
            .save_file()
        else {
            return;
        };
        if self.renaming_document.is_some() {
            self.finish_rename();
        }
        self.flush_all_now();
        match self.workspace.export_notes(&path) {
            Ok(()) => self.report_success(format!(
                "Exported {} notes to {}",
                self.workspace.documents.len(),
                path.display()
            )),
            Err(error) => self.report_error(format!("Could not export notes: {error}")),
        }
    }

    pub(in crate::app) fn import_notes(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import Goatpad notes")
            .add_filter("Goatpad notes", &["json"])
            .pick_file()
        else {
            return;
        };
        if self.renaming_document.is_some() {
            self.finish_rename();
        }
        self.capture_active_tab_state();
        self.flush_all_now();
        match self.workspace.import_notes(&path) {
            Ok(count) => {
                self.workspace_index_dirty = false;
                self.save_session();
                self.report_success(format!("Imported {count} notes from {}", path.display()));
            }
            Err(error) => self.report_error(format!("Could not import notes: {error}")),
        }
    }

    pub(in crate::app) fn activate_tab(&mut self, id: Uuid) {
        self.clear_multi_cursors();
        if self.workspace.document(id).is_none() {
            return;
        }
        if self.renaming_document.is_some() {
            self.finish_rename();
        }
        let changed = self.session.active_tab != Some(id);
        if changed {
            self.capture_active_tab_state();
            self.flush_active_now();
        }
        self.session.open_tab(id);
        self.workspace.set_active_by_id(id);
        if let Err(error) = self.workspace.touch_document(id) {
            self.report_error(format!("Could not update note activity: {error}"));
        }
        let state = self.session.tab_state.get(&id).copied().unwrap_or_default();
        self.cursor_offset = state.cursor_offset;
        self.scroll_offset = state.scroll_offset;
        self.restore_cursor = true;
        self.last_edit = None;
        self.dirty_since = None;
        self.save_session();
    }

    pub(in crate::app) fn close_tab(&mut self, id: Uuid) {
        self.clear_multi_cursors();
        if !self.session.open_tabs.contains(&id) {
            return;
        }
        if self.renaming_document == Some(id) {
            self.finish_rename();
        }
        let was_active = self.session.active_tab == Some(id);
        if was_active {
            self.capture_active_tab_state();
            self.flush_active_now();
        }
        self.session.close_tab(id);
        if was_active {
            if let Some(next_id) = self.session.active_tab {
                self.workspace.set_active_by_id(next_id);
                if let Err(error) = self.workspace.touch_document(next_id) {
                    self.report_error(format!("Could not update note activity: {error}"));
                }
                let state = self
                    .session
                    .tab_state
                    .get(&next_id)
                    .copied()
                    .unwrap_or_default();
                self.cursor_offset = state.cursor_offset;
                self.scroll_offset = state.scroll_offset;
                self.restore_cursor = true;
            } else {
                self.cursor_offset = 0;
                self.scroll_offset = 0.0;
                self.restore_cursor = false;
            }
            self.last_edit = None;
            self.dirty_since = None;
        }
        self.save_session();
    }

    pub(in crate::app) fn delete_note(&mut self, id: Uuid) {
        if self.session.open_tabs.contains(&id) {
            self.close_tab(id);
        }
        match self.workspace.delete_note(id) {
            Ok(true) => {
                self.session.tab_state.remove(&id);
                self.save_session();
            }
            Ok(false) => {}
            Err(error) => self.report_error(format!("Could not delete note: {error}")),
        }
    }

    pub(in crate::app) fn create_tab(&mut self) {
        self.clear_multi_cursors();
        if self.renaming_document.is_some() {
            self.finish_rename();
        }
        self.capture_active_tab_state();
        self.flush_active_now();
        match self.workspace.new_tab() {
            Ok(id) => {
                self.session.open_tab(id);
                self.workspace.set_active_by_id(id);
                self.cursor_offset = 0;
                self.scroll_offset = 0.0;
                self.restore_cursor = true;
                self.last_edit = None;
                self.dirty_since = None;
                self.save_session();
            }
            Err(error) => self.report_error(format!("Could not create tab: {error}")),
        }
    }
}
