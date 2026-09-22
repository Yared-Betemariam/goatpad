use super::super::*;
use super::layout::{ACTION_BAR_HEIGHT, COMPACT_ACTION_BAR_WIDTH};

impl GoatpadApp {
    pub(super) fn show_action_bar(
        &mut self,
        ui: &mut egui::Ui,
        active_is_markdown: bool,
        markdown_preview_active: bool,
    ) {
        let ctx = ui.ctx().clone();
        egui::Panel::top("action_bar")
            .exact_size(ACTION_BAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(ui.style().visuals.panel_fill)
                    .inner_margin(egui::Margin {
                        left: 10,
                        right: 10,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show(ui, |ui| {
                // ui.visuals_mut().button_frame = false;
                let widgets = &mut ui.visuals_mut().widgets;
                widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                widgets.inactive.bg_stroke = egui::Stroke::NONE;
                // optional: also flatten it while the menu is open, not just at rest
                widgets.open.weak_bg_fill = egui::Color32::TRANSPARENT;
                widgets.open.bg_stroke = egui::Stroke::NONE;
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                ui.spacing_mut().button_padding = egui::vec2(7.0, 4.0);

                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_height(ACTION_BAR_HEIGHT);

                    // Region 1: Actions
                    if ui
                        .add(
                            egui::Button::new(egui_phosphor::regular::LIST)
                                .min_size(egui::vec2(24.0, 24.0))
                                .frame_when_inactive(false),
                        )
                        .on_hover_text("Tabs list")
                        .clicked()
                    {
                        self.toggle_tabs_list();
                    }
                    ui.menu_button("File", |ui| {
                        if ui.button("New tab").clicked() {
                            self.create_tab();
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                self.session.active_tab.is_some(),
                                egui::Button::new("Close tab"),
                            )
                            .clicked()
                        {
                            if let Some(id) = self.session.active_tab {
                                self.close_tab(id);
                            }
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Tabs list").clicked() {
                            self.set_tabs_list_open(true);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Import notes").clicked() {
                            self.import_notes();
                            ui.close();
                        }
                        if ui.button("Export notes").clicked() {
                            self.export_notes();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Settings").clicked() {
                            self.settings_open = true;
                            ui.close();
                        }
                        if ui.button("Check for updates").clicked() {
                            self.settings_open = true;
                            self.settings_tab = SettingsTab::Updates;
                            self.check_for_updates();
                            ui.close();
                        }
                    });
                    ui.menu_button("Edit", |ui| {
                        if ui.button("Find in note").on_hover_text("Ctrl+F").clicked() {
                            self.open_find(FindScope::ActiveNote);
                            ui.close();
                        }
                        if ui
                            .button("Find in all notes")
                            .on_hover_text("Ctrl+Shift+F")
                            .clicked()
                        {
                            self.open_find(FindScope::AllNotes);
                            ui.close();
                        }
                        ui.separator();
                        let enabled = active_is_markdown;
                        if ui
                            .add_enabled(enabled, egui::Button::new("Bold"))
                            .on_hover_text("Ctrl+B")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::ToggleBold);
                            ui.close();
                        }
                        if ui
                            .add_enabled(enabled, egui::Button::new("Italic"))
                            .on_hover_text("Ctrl+I")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::ToggleItalic);
                            ui.close();
                        }
                        if ui
                            .add_enabled(enabled, egui::Button::new("Underline"))
                            .on_hover_text("Ctrl+U")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::ToggleUnderline);
                            ui.close();
                        }
                        if ui
                            .add_enabled(enabled, egui::Button::new("Strikethrough"))
                            .on_hover_text("Ctrl+Shift+X")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::ToggleStrikethrough);
                            ui.close();
                        }
                        ui.separator();
                        if ui
                            .add_enabled(enabled, egui::Button::new("Bulleted list"))
                            .on_hover_text("Ctrl+Shift+8")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::ToggleBulletList);
                            ui.close();
                        }
                        if ui
                            .add_enabled(enabled, egui::Button::new("Numbered list"))
                            .on_hover_text("Ctrl+Shift+7")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::ToggleNumberedList);
                            ui.close();
                        }
                        ui.separator();
                        if ui
                            .add_enabled(enabled, egui::Button::new("Link…"))
                            .on_hover_text("Ctrl+K")
                            .clicked()
                        {
                            self.apply_formatting(&ctx, Action::InsertLink);
                            ui.close();
                        }
                        if ui
                            .add_enabled(enabled, egui::Button::new("Clear formatting"))
                            .clicked()
                        {
                            self.apply_clear_formatting(&ctx);
                            ui.close();
                        }
                    });
                    ui.menu_button("View", |ui| {
                        if let Some(document_id) = self.session.active_tab {
                            let current_kind = self
                                .workspace
                                .document(document_id)
                                .map(|document| document.kind);
                            if ui
                                .selectable_label(
                                    current_kind == Some(DocKind::Md),
                                    "Markdown document",
                                )
                                .clicked()
                            {
                                self.flush_active_now();
                                if let Err(error) =
                                    self.workspace.set_document_kind(document_id, DocKind::Md)
                                {
                                    self.report_error(format!(
                                        "Could not change document type: {error}"
                                    ));
                                }
                                ui.close();
                            }
                            if ui
                                .selectable_label(
                                    current_kind == Some(DocKind::Txt),
                                    "Plain text document",
                                )
                                .clicked()
                            {
                                self.flush_active_now();
                                if let Err(error) =
                                    self.workspace.set_document_kind(document_id, DocKind::Txt)
                                {
                                    self.report_error(format!(
                                        "Could not change document type: {error}"
                                    ));
                                }
                                ui.close();
                            }
                            ui.separator();
                        }
                        let spellcheck_available = self.spellchecker.is_available();
                        let spellcheck_response = ui
                            .add_enabled_ui(spellcheck_available, |ui| {
                                ui.selectable_label(
                                    self.settings.spellcheck_enabled,
                                    "Check spelling",
                                )
                            })
                            .inner
                            .on_hover_text(if spellcheck_available {
                                "Underline misspelled words in red, using Windows' spell checker"
                            } else {
                                "Windows' spell checker is unavailable on this system"
                            });
                        if spellcheck_response.clicked() {
                            self.settings.spellcheck_enabled = !self.settings.spellcheck_enabled;
                            self.save_settings();
                            ui.close();
                        }
                        if active_is_markdown {
                            let preview_highlighting_response = ui
                                .selectable_label(
                                    self.settings.markdown_preview_highlighting_enabled,
                                    "Color highlighting in Markdown preview",
                                )
                                .on_hover_text(
                                    "Use the theme primary and secondary colors in Markdown preview",
                                );
                            if preview_highlighting_response.clicked() {
                                self.settings.markdown_preview_highlighting_enabled =
                                    !self.settings.markdown_preview_highlighting_enabled;
                                self.save_settings();
                                ui.close();
                            }
                        }
                        ui.separator();
                        ui.menu_button("Theme", |ui| {
                            for theme in self.themes.clone() {
                                if ui
                                    .selectable_label(
                                        theme.name == self.settings.theme,
                                        theme.display_name(),
                                    )
                                    .clicked()
                                {
                                    self.select_theme(&ctx, theme);
                                    ui.close();
                                }
                            }
                        });
                        ui.separator();
                        if ui.button("Zoom in").clicked() {
                            self.set_content_zoom(self.zoom + CONTENT_ZOOM_STEP);
                            ui.close();
                        }
                        if ui.button("Zoom out").clicked() {
                            self.set_content_zoom(self.zoom - CONTENT_ZOOM_STEP);
                            ui.close();
                        }
                        if ui.button("Reset zoom").clicked() {
                            self.set_content_zoom(DEFAULT_CONTENT_ZOOM);
                            ui.close();
                        }
                    });

                    ui.add_space(36.0);

                    // Region 2: Markdown options (only rendered when active note is MD)
                    if active_is_markdown {
                        let available_width = ui.available_width();
                        if available_width < COMPACT_ACTION_BAR_WIDTH {
                            ui.menu_button("Format", |ui| {
                                ui.menu_button(
                                    format!("{} Headings", egui_phosphor::regular::TEXT_H),
                                    |ui| {
                                        if ui
                                            .button(format!(
                                                "{} Heading 1",
                                                egui_phosphor::regular::TEXT_H_ONE
                                            ))
                                            .clicked()
                                        {
                                            self.apply_heading(&ctx, 1);
                                            ui.close();
                                        }
                                        if ui
                                            .button(format!(
                                                "{} Heading 2",
                                                egui_phosphor::regular::TEXT_H_TWO
                                            ))
                                            .clicked()
                                        {
                                            self.apply_heading(&ctx, 2);
                                            ui.close();
                                        }
                                        if ui
                                            .button(format!(
                                                "{} Heading 3",
                                                egui_phosphor::regular::TEXT_H_THREE
                                            ))
                                            .clicked()
                                        {
                                            self.apply_heading(&ctx, 3);
                                            ui.close();
                                        }
                                    },
                                );
                                ui.menu_button(
                                    format!("{} Lists", egui_phosphor::regular::LIST_BULLETS),
                                    |ui| {
                                        if ui
                                            .button(format!(
                                                "{} Bulleted list",
                                                egui_phosphor::regular::LIST_BULLETS
                                            ))
                                            .clicked()
                                        {
                                            self.apply_formatting(&ctx, Action::ToggleBulletList);
                                            ui.close();
                                        }
                                        if ui
                                            .button(format!(
                                                "{} Numbered list",
                                                egui_phosphor::regular::LIST_NUMBERS
                                            ))
                                            .clicked()
                                        {
                                            self.apply_formatting(&ctx, Action::ToggleNumberedList);
                                            ui.close();
                                        }
                                    },
                                );
                                ui.separator();
                                if ui
                                    .button(format!(
                                        "{} Bold (Ctrl+B)",
                                        egui_phosphor::regular::TEXT_B
                                    ))
                                    .clicked()
                                {
                                    self.apply_formatting(&ctx, Action::ToggleBold);
                                    ui.close();
                                }
                                if ui
                                    .button(format!(
                                        "{} Italic (Ctrl+I)",
                                        egui_phosphor::regular::TEXT_ITALIC
                                    ))
                                    .clicked()
                                {
                                    self.apply_formatting(&ctx, Action::ToggleItalic);
                                    ui.close();
                                }
                                if ui
                                    .button(format!(
                                        "{} Strikethrough (Ctrl+Shift+X)",
                                        egui_phosphor::regular::TEXT_STRIKETHROUGH
                                    ))
                                    .clicked()
                                {
                                    self.apply_formatting(&ctx, Action::ToggleStrikethrough);
                                    ui.close();
                                }
                                ui.separator();
                                if ui
                                    .button(format!(
                                        "{} Link… (Ctrl+K)",
                                        egui_phosphor::regular::LINK
                                    ))
                                    .clicked()
                                {
                                    self.apply_formatting(&ctx, Action::InsertLink);
                                    ui.close();
                                }
                                if ui
                                    .button(format!("{} Table", egui_phosphor::regular::TABLE))
                                    .clicked()
                                {
                                    self.apply_table_insert(&ctx);
                                    ui.close();
                                }
                                ui.separator();
                                if ui
                                    .button(format!(
                                        "{} Clear formatting",
                                        egui_phosphor::regular::ERASER
                                    ))
                                    .clicked()
                                {
                                    self.apply_clear_formatting(&ctx);
                                    ui.close();
                                }
                            });
                        } else {
                            ui.menu_button(
                                format!("{} H1", egui_phosphor::regular::TEXT_H),
                                |ui| {
                                    if ui
                                        .button(format!(
                                            "{} Heading 1",
                                            egui_phosphor::regular::TEXT_H_ONE
                                        ))
                                        .clicked()
                                    {
                                        self.apply_heading(&ctx, 1);
                                        ui.close();
                                    }
                                    if ui
                                        .button(format!(
                                            "{} Heading 2",
                                            egui_phosphor::regular::TEXT_H_TWO
                                        ))
                                        .clicked()
                                    {
                                        self.apply_heading(&ctx, 2);
                                        ui.close();
                                    }
                                    if ui
                                        .button(format!(
                                            "{} Heading 3",
                                            egui_phosphor::regular::TEXT_H_THREE
                                        ))
                                        .clicked()
                                    {
                                        self.apply_heading(&ctx, 3);
                                        ui.close();
                                    }
                                },
                            );
                            ui.menu_button(egui_phosphor::regular::LIST_BULLETS, |ui| {
                                if ui
                                    .button(format!(
                                        "{} Bulleted list",
                                        egui_phosphor::regular::LIST_BULLETS
                                    ))
                                    .clicked()
                                {
                                    self.apply_formatting(&ctx, Action::ToggleBulletList);
                                    ui.close();
                                }
                                if ui
                                    .button(format!(
                                        "{} Numbered list",
                                        egui_phosphor::regular::LIST_NUMBERS
                                    ))
                                    .clicked()
                                {
                                    self.apply_formatting(&ctx, Action::ToggleNumberedList);
                                    ui.close();
                                }
                            });
                            if ui
                                .button(egui_phosphor::regular::TEXT_B)
                                .on_hover_text("Bold (Ctrl+B)")
                                .clicked()
                            {
                                self.apply_formatting(&ctx, Action::ToggleBold);
                            }
                            if ui
                                .button(egui_phosphor::regular::TEXT_ITALIC)
                                .on_hover_text("Italic (Ctrl+I)")
                                .clicked()
                            {
                                self.apply_formatting(&ctx, Action::ToggleItalic);
                            }
                            if ui
                                .button(egui_phosphor::regular::TEXT_STRIKETHROUGH)
                                .on_hover_text("Strike-through (Ctrl+Shift+X)")
                                .clicked()
                            {
                                self.apply_formatting(&ctx, Action::ToggleStrikethrough);
                            }
                            if ui
                                .button(egui_phosphor::regular::LINK)
                                .on_hover_text("Link (Ctrl+K)")
                                .clicked()
                            {
                                self.apply_formatting(&ctx, Action::InsertLink);
                            }
                            if ui
                                .button(egui_phosphor::regular::TABLE)
                                .on_hover_text("Table")
                                .clicked()
                            {
                                self.apply_table_insert(&ctx);
                            }
                            if ui
                                .button(egui_phosphor::regular::ERASER)
                                .on_hover_text("Clear formatting")
                                .clicked()
                            {
                                self.apply_clear_formatting(&ctx);
                            }
                        }
                    }

                    ui.add_space(36.0);

                    // Region 3: MD/TXT Switcher (right-aligned)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x -= 7.0;
                        ui.spacing_mut().button_padding = egui::vec2(8.0, 2.0);

                        if let Some(document_id) = self.session.active_tab {
                            if let Some(current_kind) = self
                                .workspace
                                .document(document_id)
                                .map(|document| document.kind)
                            {
                                let mut requested_kind = current_kind;
                                let primary = self.theme_draft.primary.0;
                                let background = self.theme_draft.background.0;
                                let normal_text = ui.visuals().text_color();
                                let inactive_text = normal_text.gamma_multiply(0.6);
                                let switcher_stroke =
                                    egui::Stroke::new(1.0, self.theme_draft.border_color());

                                ui.add(
                                    egui::Button::new(egui::RichText::new("TXT").color(
                                        if requested_kind == DocKind::Txt {
                                            primary
                                        } else {
                                            inactive_text
                                        },
                                    ))
                                    .fill(if requested_kind == DocKind::Txt {
                                        primary.gamma_multiply(0.15)
                                    } else {
                                        background
                                    })
                                    .stroke(switcher_stroke)
                                    .corner_radius(
                                        egui::CornerRadius {
                                            nw: 0,
                                            ne: 5,
                                            sw: 0,
                                            se: 5,
                                        },
                                    ),
                                )
                                .clicked()
                                .then(|| requested_kind = DocKind::Txt);
                                ui.add(
                                    egui::Button::new(egui::RichText::new("MD").color(
                                        if requested_kind == DocKind::Md {
                                            primary
                                        } else {
                                            inactive_text
                                        },
                                    ))
                                    .fill(if requested_kind == DocKind::Md {
                                        primary.gamma_multiply(0.15)
                                    } else {
                                        background
                                    })
                                    .stroke(switcher_stroke)
                                    .corner_radius(
                                        egui::CornerRadius {
                                            nw: 5,
                                            ne: 0,
                                            sw: 5,
                                            se: 0,
                                        },
                                    ),
                                )
                                .clicked()
                                .then(|| requested_kind = DocKind::Md);

                                if current_kind == DocKind::Md {
                                    ui.add_space(6.0);
                                }

                                if current_kind == DocKind::Md
                                    && ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new(egui_phosphor::regular::EYE)
                                                    .color(if markdown_preview_active {
                                                        primary
                                                    } else {
                                                        inactive_text
                                                    }),
                                            )
                                            .fill(if markdown_preview_active {
                                                primary.gamma_multiply(0.15)
                                            } else {
                                                background
                                            })
                                            .stroke(switcher_stroke)
                                            .min_size(egui::vec2(24.0, 24.0))
                                            .corner_radius(egui::CornerRadius {
                                                nw: 5,
                                                ne: 5,
                                                sw: 5,
                                                se: 5,
                                            }),
                                        )
                                        .on_hover_text(if markdown_preview_active {
                                            "Exit Markdown preview and unlock editing"
                                        } else {
                                            "Preview Markdown and lock editing"
                                        })
                                        .clicked()
                                {
                                    self.toggle_active_markdown_preview();
                                }

                                if requested_kind != current_kind {
                                    self.flush_active_now();
                                    if let Err(error) = self
                                        .workspace
                                        .set_document_kind(document_id, requested_kind)
                                    {
                                        self.report_error(format!(
                                            "Could not change document type: {error}"
                                        ));
                                    }
                                }
                            }
                        }
                    });
                });
            });
    }
}
