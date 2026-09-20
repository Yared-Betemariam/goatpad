use super::*;

impl GoatpadApp {
    pub(in crate::app) fn open_find(&mut self, scope: FindScope) {
        self.clear_multi_cursors();
        match self.find.as_mut() {
            Some(find) => {
                find.scope = scope;
                find.focus_input = true;
                find.current = None;
            }
            None => {
                self.find = Some(FindState {
                    scope,
                    query: String::new(),
                    focus_input: true,
                    current: None,
                });
            }
        }
    }

    pub(in crate::app) fn current_find_matches(&self) -> Vec<FindMatch> {
        let Some(find) = self.find.as_ref() else {
            return Vec::new();
        };
        if find.query.is_empty() {
            return Vec::new();
        }

        match find.scope {
            FindScope::ActiveNote => self
                .session
                .active_tab
                .and_then(|id| self.workspace.document(id))
                .into_iter()
                .flat_map(|document| {
                    editor_find::find_ranges(&document.content, &find.query)
                        .into_iter()
                        .map(move |range| FindMatch {
                            document_id: document.id,
                            range,
                        })
                })
                .collect(),
            FindScope::AllNotes => self
                .workspace
                .documents
                .iter()
                .flat_map(|document| {
                    editor_find::find_ranges(&document.content, &find.query)
                        .into_iter()
                        .map(move |range| FindMatch {
                            document_id: document.id,
                            range,
                        })
                })
                .collect(),
        }
    }

    pub(in crate::app) fn visible_find_ranges(&self, document_id: Uuid) -> Vec<Range<usize>> {
        self.current_find_matches()
            .into_iter()
            .filter(|matched| matched.document_id == document_id)
            .map(|matched| matched.range)
            .collect()
    }

    pub(in crate::app) fn navigate_find(&mut self, ctx: &egui::Context, forward: bool) {
        self.clear_multi_cursors();
        let matches = self.current_find_matches();
        if matches.is_empty() {
            if let Some(find) = self.find.as_mut() {
                find.current = None;
            }
            return;
        }

        let current = self.find.as_ref().and_then(|find| find.current);
        let next = match (current, forward) {
            (Some(index), true) => (index + 1) % matches.len(),
            (Some(0), false) | (None, false) => matches.len() - 1,
            (Some(index), false) => index - 1,
            (None, true) => 0,
        };
        let matched = matches[next].clone();
        if let Some(find) = self.find.as_mut() {
            find.current = Some(next);
        }
        if self.session.active_tab != Some(matched.document_id) {
            self.activate_tab(matched.document_id);
        }

        let document = self.workspace.active_document();
        let start = document.content[..matched.range.start].chars().count();
        let end = start + document.content[matched.range].chars().count();
        let editor_id = self.editor_id();
        let mut state =
            egui::widgets::text_edit::TextEditState::load(ctx, editor_id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(start),
                egui::text::CCursor::new(end),
            )));
        state.store(ctx, editor_id);
        self.cursor_offset = end;
        self.restore_cursor = false;
        self.pending_find_scroll = Some((matched.document_id, start));
        ctx.memory_mut(|memory| memory.request_focus(editor_id));
    }

    pub(in crate::app) fn close_find(&mut self, ctx: &egui::Context) {
        self.find = None;
        if self.session.active_tab.is_some() {
            let editor_id = self.editor_id();
            ctx.memory_mut(|memory| memory.request_focus(editor_id));
        }
    }
}
