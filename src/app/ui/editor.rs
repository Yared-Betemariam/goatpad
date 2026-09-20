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
                if self.restore_cursor {
                    let offset = self
                        .cursor_offset
                        .min(self.workspace.active_document().content.chars().count());
                    let mut state = egui::widgets::text_edit::TextEditState::load(ctx, editor_id)
                        .unwrap_or_default();
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(offset),
                        )));
                    state.store(ctx, editor_id);
                    self.restore_cursor = false;
                }
                let before_content = (!self.multi_cursor_offsets.is_empty())
                    .then(|| self.workspace.active_document().content.clone());
                let before_cursor = self.editor_cursor_range(ctx);
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
                let primary_cursor_after = editor.cursor_range.map(|range| range.primary.index.0);
                let clicked_cursor = editor.response.interact_pointer_pos().map(|pos| {
                    editor
                        .galley
                        .cursor_from_pos(pos - editor.galley_pos)
                        .index
                        .0
                });
                let alt_click = editor.response.clicked() && ctx.input(|input| input.modifiers.alt);
                let history_event = ctx.input(|input| {
                    input.events.iter().any(|event| {
                        matches!(
                            event,
                            egui::Event::Key {
                                key: egui::Key::Z | egui::Key::Y,
                                pressed: true,
                                modifiers,
                                ..
                            } if modifiers.command
                        )
                    })
                });
                if let Some(cursor_range) = editor.cursor_range {
                    self.cursor_offset = cursor_range.primary.index.0;
                }
                if alt_click {
                    if let Some(clicked_offset) = clicked_cursor.or(primary_cursor_after) {
                        self.multi_cursor_offsets
                            .retain(|offset| *offset != clicked_offset);
                        self.multi_cursor_offsets.push(clicked_offset);
                        self.multi_cursor_offsets.sort_unstable();
                        self.multi_cursor_offsets.dedup();
                    }
                } else if editor.response.clicked() {
                    self.clear_multi_cursors();
                } else if editor.response.changed() {
                    if history_event {
                        // egui's undo stack restores the complete pre-edit
                        // string, including edits mirrored at secondary carets.
                        self.clear_multi_cursors();
                    } else if let (Some(before_content), Some(primary_cursor_after)) =
                        (before_content.as_deref(), primary_cursor_after)
                    {
                        self.mirror_multi_cursor_edit(
                            ctx,
                            editor.response.id,
                            before_content,
                            before_cursor,
                            primary_cursor_after,
                        );
                    }
                } else if !self.multi_cursor_offsets.is_empty()
                    && editor
                        .cursor_range
                        .is_some_and(|range| range != before_cursor)
                {
                    let navigation = ctx.input(|input| {
                        input.events.iter().rev().find_map(|event| {
                            if let egui::Event::Key {
                                key,
                                pressed: true,
                                modifiers,
                                ..
                            } = event
                            {
                                Some((*key, *modifiers))
                            } else {
                                None
                            }
                        })
                    });
                    let moved = navigation.zip(before_content.as_deref()).and_then(
                        |((key, modifiers), content)| {
                            primary_cursor_after.and_then(|primary_cursor_after| {
                                multicursor::move_secondary_offsets(
                                    content,
                                    before_cursor,
                                    primary_cursor_after,
                                    &self.multi_cursor_offsets,
                                    key,
                                    modifiers,
                                )
                            })
                        },
                    );
                    if let Some(moved) = moved {
                        self.multi_cursor_offsets = moved;
                    } else {
                        self.clear_multi_cursors();
                    }
                }
                if editor.response.changed() {
                    self.mark_active_document_edited();
                }
                if !self.multi_cursor_offsets.is_empty() && editor.response.has_focus() {
                    let painter = ui.painter_at(editor.text_clip_rect);
                    let cursor_color = ui.visuals().selection.stroke.color;
                    let max_offset = editor.galley.job.text.chars().count();
                    for offset in &self.multi_cursor_offsets {
                        let cursor = editor
                            .galley
                            .pos_from_cursor(egui::text::CCursor::new((*offset).min(max_offset)));
                        let cursor = cursor.translate(
                            editor.galley_pos.to_vec2()
                                - egui::vec2(editor.galley.rect.left(), 0.0),
                        );
                        painter.line_segment(
                            [
                                egui::pos2(cursor.left(), cursor.top()),
                                egui::pos2(cursor.left(), cursor.bottom()),
                            ],
                            egui::Stroke::new(1.25, cursor_color),
                        );
                    }
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
