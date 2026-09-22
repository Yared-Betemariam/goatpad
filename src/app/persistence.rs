use super::*;

impl GoatpadApp {
    pub(in crate::app) fn poll_writer_results(&mut self) {
        loop {
            match self.writer_results.try_recv() {
                Ok(SaveResult {
                    id: _,
                    result: Ok(()),
                }) => {}
                Ok(SaveResult {
                    id,
                    result: Err(error),
                }) => {
                    if let Some(document) =
                        self.workspace.documents.iter_mut().find(|doc| doc.id == id)
                    {
                        document.dirty = true;
                    }
                    self.last_edit = Some(Instant::now());
                    self.report_error(error);
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
    }

    pub(in crate::app) fn mark_active_document_edited(&mut self) {
        let title_changed = {
            let document = self.workspace.active_document_mut();
            document.dirty = true;
            document.refresh_automatic_title()
        };
        self.workspace_index_dirty |= title_changed;
        let now = Instant::now();
        self.last_edit = Some(now);
        self.dirty_since.get_or_insert(now);
    }

    pub(in crate::app) fn flush_workspace_index(&mut self) {
        if !self.workspace_index_dirty {
            return;
        }
        match self.workspace.save_index() {
            Ok(()) => self.workspace_index_dirty = false,
            Err(error) => self.report_error(format!("Could not save note titles: {error}")),
        }
    }

    pub(in crate::app) fn queue_active_save(&mut self) {
        let Some(active_id) = self.session.active_tab else {
            return;
        };
        let Some(document) = self.workspace.document(active_id) else {
            return;
        };
        if !document.dirty {
            return;
        }
        let request = SaveRequest {
            id: document.id,
            kind: document.kind,
            content: document.content.clone(),
            path: self.workspace.document_path(document.id, document.kind),
        };
        if self.writer.send(request).is_ok() {
            self.workspace.active_document_mut().dirty = false;
            self.last_edit = None;
            self.dirty_since = None;
            self.flush_workspace_index();
        }
    }

    pub(in crate::app) fn flush_active_now(&mut self) {
        let Some(active_id) = self.session.active_tab else {
            self.flush_workspace_index();
            return;
        };
        let Some(index) = self
            .workspace
            .documents
            .iter()
            .position(|document| document.id == active_id)
        else {
            self.flush_workspace_index();
            return;
        };
        if self.workspace.documents[index].dirty {
            if let Err(error) = self
                .workspace
                .save_document(&self.workspace.documents[index])
            {
                self.report_error(format!("Could not save document: {error}"));
            } else {
                self.workspace.documents[index].dirty = false;
            }
        }
        self.flush_workspace_index();
    }

    pub(in crate::app) fn flush_all_now(&mut self) {
        for index in 0..self.workspace.documents.len() {
            if self.workspace.documents[index].dirty {
                if let Err(error) = self
                    .workspace
                    .save_document(&self.workspace.documents[index])
                {
                    self.report_error(format!("Could not save document: {error}"));
                } else {
                    self.workspace.documents[index].dirty = false;
                }
            }
        }
        self.flush_workspace_index();
    }

    pub(in crate::app) fn capture_active_tab_state(&mut self) {
        let Some(id) = self.session.active_tab else {
            return;
        };
        self.session.tab_state.insert(
            id,
            TabState {
                cursor_offset: self.cursor_offset,
                scroll_offset: self.scroll_offset,
            },
        );
    }

    pub(in crate::app) fn save_session(&mut self) {
        self.capture_active_tab_state();
        let folder_ids = self
            .workspace
            .folders
            .iter()
            .map(|folder| folder.id)
            .collect::<HashSet<_>>();
        self.session.expanded_folders = self
            .expanded_folders
            .iter()
            .filter(|id| folder_ids.contains(id))
            .copied()
            .collect();
        if let Err(error) = self.session.save(&self.paths) {
            self.report_error(format!("Could not save session: {error}"));
        } else {
            self.last_session_save = Instant::now();
        }
    }
}
