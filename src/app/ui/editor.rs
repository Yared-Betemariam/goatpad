use super::super::*;
use super::layout::{DOCUMENT_VIEW_HORIZONTAL_PADDING, DOCUMENT_VIEW_VERTICAL_PADDING};

impl GoatpadApp {
    pub(super) fn show_editor(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(ui.style().visuals.panel_fill)
                    .inner_margin(egui::Margin {
                        top: 0,
                        bottom: 0,
                        right: 6,
                        left: 6,
                    }),
            )
            .show(ui, |ui| {
                let Some(document_id) = self.session.active_tab else {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            "No tabs open — create a new note, or pick one from the tabs list.",
                        );
                    });
                    return;
                };
                if !self.workspace.set_active_by_id(document_id) {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            "No tabs open — create a new note, or pick one from the tabs list.",
                        );
                    });
                    return;
                }
                let is_markdown = self.workspace.active_document().kind == DocKind::Md;
                let preview_active =
                    is_markdown && self.session.markdown_previews.contains(&document_id);
                let zoom = self.zoom;
                let font_family = self.theme_draft.content_font_family();
                let dark_mode = ui.visuals().dark_mode;
                let text_color = if dark_mode {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::BLACK
                };
                if preview_active {
                    let output = egui::ScrollArea::vertical()
                        .id_salt(("markdown-preview-scroll", document_id))
                        .auto_shrink([false, true])
                        .content_margin(egui::Margin {
                            top: DOCUMENT_VIEW_VERTICAL_PADDING,
                            bottom: DOCUMENT_VIEW_VERTICAL_PADDING,
                            right: DOCUMENT_VIEW_HORIZONTAL_PADDING,
                            left: DOCUMENT_VIEW_HORIZONTAL_PADDING,
                        })
                        .vertical_scroll_offset(self.scroll_offset)
                        .show(ui, |ui| {
                            super::markdown_preview::render(
                                ui,
                                &self.workspace.active_document().content,
                                zoom,
                                &font_family,
                                text_color,
                            );
                        });
                    self.scroll_offset = output.state.offset.y;
                    return;
                }
                let editor_id = self.editor_id();
                let misspelled_ranges = self.spellcheck_ranges(document_id);
                let find_ranges = self.visible_find_ranges(document_id);
                let layouter_misspelled = misspelled_ranges.clone();
                let layouter_find_ranges = find_ranges.clone();
                let mut layouter =
                    move |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, wrap_width: f32| {
                        let mut job = if is_markdown {
                            highlighting::highlight(
                                buffer.as_str(),
                                zoom,
                                &font_family,
                                text_color,
                                dark_mode,
                                &layouter_misspelled,
                                &layouter_find_ranges,
                            )
                        } else {
                            highlighting::plain(
                                buffer.as_str(),
                                zoom,
                                &font_family,
                                text_color,
                                &layouter_misspelled,
                                &layouter_find_ranges,
                            )
                        };
                        job.wrap.max_width = wrap_width;
                        ui.fonts_mut(|fonts| fonts.layout_job(job))
                    };
                let scroll_to_find = self
                    .pending_find_scroll
                    .take()
                    .filter(|(target_id, _)| *target_id == document_id)
                    .map(|(_, char_index)| char_index);
                let output = egui::ScrollArea::vertical()
                    .id_salt(("editor-scroll", document_id))
                    .content_margin(egui::Margin {
                        top: DOCUMENT_VIEW_VERTICAL_PADDING,
                        bottom: DOCUMENT_VIEW_VERTICAL_PADDING,
                        right: DOCUMENT_VIEW_HORIZONTAL_PADDING,
                        left: DOCUMENT_VIEW_HORIZONTAL_PADDING,
                    })
                    .vertical_scroll_offset(self.scroll_offset)
                    .show(ui, |ui| {
                        let available_height = ui.available_height();
                        let line_height = 20.0 * zoom;
                        let editor = egui::TextEdit::multiline(
                            &mut self.workspace.active_document_mut().content,
                        )
                        .id(editor_id)
                        .desired_width(ui.available_width())
                        .desired_rows((available_height / line_height).ceil().max(1.0) as usize)
                        .lock_focus(true)
                        .frame(egui::Frame::NONE)
                        .layouter(&mut layouter)
                        .show(ui);
                        if let Some(char_index) = scroll_to_find {
                            let match_rect = editor
                                .galley
                                .pos_from_cursor(egui::text::CCursor::new(char_index))
                                .translate(editor.galley_pos.to_vec2())
                                .expand(8.0);
                            ui.scroll_to_rect(match_rect, Some(egui::Align::Center));
                        }
                        editor
                    });
                self.scroll_offset = output.state.offset.y;
                let editor = output.inner;
                if self.restore_cursor {
                    let offset = self
                        .cursor_offset
                        .min(self.workspace.active_document().content.chars().count());
                    let mut state =
                        egui::widgets::text_edit::TextEditState::load(ctx, editor.response.id)
                            .unwrap_or_default();
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(offset),
                        )));
                    state.store(ctx, editor.response.id);
                    self.restore_cursor = false;
                }
                if let Some(cursor_range) = editor.cursor_range {
                    self.cursor_offset = cursor_range.primary.index.0;
                }
                if editor.response.changed() {
                    self.mark_active_document_edited();
                }
                if editor.response.secondary_clicked() {
                    let clicked_target = editor.response.interact_pointer_pos().and_then(|pos| {
                        let local = pos - editor.galley_pos;
                        let char_index = editor.galley.cursor_from_pos(local).index.0;
                        let content = &self.workspace.active_document().content;
                        Self::spellcheck_target_at(content, &misspelled_ranges, char_index)
                    });
                    self.spellcheck_menu = clicked_target.map(|range| {
                        let word =
                            self.workspace.active_document().content[range.clone()].to_owned();
                        let suggestions = self.spellchecker.suggestions(&word);
                        SpellcheckMenuTarget {
                            document_id,
                            range,
                            word,
                            suggestions,
                        }
                    });
                }
                if self.spellcheck_menu.is_some() {
                    editor.response.context_menu(|ui| {
                        self.render_spellcheck_context_menu(ui, ctx);
                    });
                }
            });
    }
}
