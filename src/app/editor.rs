use super::*;

impl GoatpadApp {
    pub(in crate::app) fn editor_id(&self) -> egui::Id {
        egui::Id::new((
            "editor",
            self.session
                .active_tab
                .expect("editor requires an active tab"),
        ))
    }

    fn selection_range(&self, ctx: &egui::Context) -> (usize, usize) {
        let editor_id = self.editor_id();
        let range = egui::widgets::text_edit::TextEditState::load(ctx, editor_id)
            .and_then(|state| state.cursor.char_range())
            .unwrap_or_else(|| {
                egui::text::CCursorRange::one(egui::text::CCursor::new(self.cursor_offset))
            });
        if range.primary.index.0 <= range.secondary.index.0 {
            (range.primary.index.0, range.secondary.index.0)
        } else {
            (range.secondary.index.0, range.primary.index.0)
        }
    }

    /// Applies a selection-transforming edit (formatting, list toggles, links, tables, …)
    /// to the active document and restores the cursor/selection afterwards.
    fn apply_text_transform(
        &mut self,
        ctx: &egui::Context,
        transform: impl FnOnce(&mut String, usize, usize) -> egui::text::CCursorRange,
    ) {
        let (start, end) = self.selection_range(ctx);
        let editor_id = self.editor_id();
        let new_range = transform(
            &mut self.workspace.active_document_mut().content,
            start,
            end,
        );
        let mut state =
            egui::widgets::text_edit::TextEditState::load(ctx, editor_id).unwrap_or_default();
        state.cursor.set_char_range(Some(new_range));
        state.store(ctx, editor_id);
        self.cursor_offset = new_range.primary.index.0;
        self.mark_active_document_edited();
    }

    pub(in crate::app) fn apply_formatting(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::ToggleBulletList => {
                self.apply_text_transform(ctx, formatting::toggle_bullet_list)
            }
            Action::ToggleNumberedList => {
                self.apply_text_transform(ctx, formatting::toggle_numbered_list)
            }
            Action::InsertLink => self.apply_text_transform(ctx, formatting::insert_link),
            Action::ToggleBold => {
                self.apply_text_transform(ctx, |text, start, end| {
                    formatting::wrap_selection(text, start, end, "**", "**")
                });
            }
            Action::ToggleItalic => {
                self.apply_text_transform(ctx, |text, start, end| {
                    formatting::wrap_selection(text, start, end, "*", "*")
                });
            }
            Action::ToggleUnderline => {
                self.apply_text_transform(ctx, |text, start, end| {
                    formatting::wrap_selection(text, start, end, "<u>", "</u>")
                });
            }
            Action::ToggleStrikethrough => {
                self.apply_text_transform(ctx, |text, start, end| {
                    formatting::wrap_selection(text, start, end, "~~", "~~")
                });
            }
            _ => {}
        }
    }

    pub(in crate::app) fn apply_heading(&mut self, ctx: &egui::Context, level: u8) {
        self.apply_text_transform(ctx, move |text, start, end| {
            formatting::set_heading(text, start, end, level)
        });
    }

    pub(in crate::app) fn apply_clear_formatting(&mut self, ctx: &egui::Context) {
        self.apply_text_transform(ctx, formatting::clear_formatting);
    }

    pub(in crate::app) fn apply_table_insert(&mut self, ctx: &egui::Context) {
        self.apply_text_transform(ctx, formatting::insert_table);
    }

    /// Recomputes (or reuses a cached) list of misspelled UTF-8 byte ranges
    /// for `document_id`'s current content, using the OS spell checker.
    pub(in crate::app) fn spellcheck_ranges(&mut self, document_id: Uuid) -> Vec<Range<usize>> {
        if !self.settings.spellcheck_enabled {
            self.spellcheck_cache = SpellcheckCache::default();
            return Vec::new();
        }
        let content_hash = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            self.workspace.active_document().content.hash(&mut hasher);
            hasher.finish()
        };
        if self.spellcheck_cache.document_id != Some(document_id)
            || self.spellcheck_cache.content_hash != content_hash
        {
            let ranges = self
                .spellchecker
                .check(&self.workspace.active_document().content);
            self.spellcheck_cache = SpellcheckCache {
                document_id: Some(document_id),
                content_hash,
                ranges,
            };
        }
        self.spellcheck_cache.ranges.clone()
    }

    /// Forces the next frame to re-run the spell checker, e.g. after editing
    /// the dictionary or replacing a misspelled word.
    fn invalidate_spellcheck_cache(&mut self) {
        self.spellcheck_cache = SpellcheckCache::default();
    }

    /// Finds the misspelled-word range (if any) under a click at `char_index`
    /// within `content`, given the currently-known misspelled ranges.
    pub(in crate::app) fn spellcheck_target_at(
        content: &str,
        misspelled: &[Range<usize>],
        char_index: usize,
    ) -> Option<Range<usize>> {
        let byte_index = formatting::byte_index(content, char_index);
        misspelled
            .iter()
            .find(|range| byte_index >= range.start && byte_index <= range.end)
            .cloned()
    }

    /// Replaces a misspelled word's byte range with `replacement`, then
    /// places the cursor immediately after it.
    fn replace_misspelled_word(
        &mut self,
        ctx: &egui::Context,
        range: Range<usize>,
        replacement: &str,
    ) {
        let editor_id = self.editor_id();
        let document = self.workspace.active_document_mut();
        if range.end > document.content.len() {
            return;
        }
        document.content.replace_range(range.clone(), replacement);
        let prefix_chars = document.content[..range.start].chars().count();
        let new_char_index = prefix_chars + replacement.chars().count();

        let mut state =
            egui::widgets::text_edit::TextEditState::load(ctx, editor_id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(new_char_index),
            )));
        state.store(ctx, editor_id);
        self.cursor_offset = new_char_index;
        self.mark_active_document_edited();
        self.invalidate_spellcheck_cache();
    }

    /// Renders the "Add to dictionary" / suggestions context menu for the
    /// word most recently right-clicked in the editor.
    pub(in crate::app) fn render_spellcheck_context_menu(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
    ) {
        let Some(target) = self.spellcheck_menu.clone() else {
            return;
        };
        if self.workspace.active_document().id != target.document_id {
            return;
        }
        if target.suggestions.is_empty() {
            ui.add_enabled(false, egui::Button::new("No spelling suggestions"));
        } else {
            for suggestion in &target.suggestions {
                if ui.button(suggestion).clicked() {
                    self.replace_misspelled_word(ctx, target.range.clone(), suggestion);
                    self.spellcheck_menu = None;
                    ui.close();
                }
            }
        }
        ui.separator();
        if ui
            .button(format!("Add \"{}\" to dictionary", target.word))
            .clicked()
        {
            self.spellchecker.add_to_dictionary(&target.word);
            self.invalidate_spellcheck_cache();
            self.spellcheck_menu = None;
            ui.close();
        }
        if ui.button("Ignore").clicked() {
            self.spellchecker.ignore(&target.word);
            self.invalidate_spellcheck_cache();
            self.spellcheck_menu = None;
            ui.close();
        }
    }
}
