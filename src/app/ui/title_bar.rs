use super::super::*;
use super::layout::{
    MIN_TABS_WIDTH, MIN_TITLE_DRAG_WIDTH, TITLE_BAR_HEIGHT, TITLE_BAR_SPACING,
    TITLE_CONTENT_HEIGHT, TITLE_CONTROL_WIDTH, TITLE_TAB_HEIGHT, WINDOW_BUTTON_WIDTH,
};

/// Sizing for a single title-bar tab, derived from the active theme's font
/// size so the tab's label, close button, and padding stay proportional and
/// unclipped within the fixed `TITLE_TAB_HEIGHT`, instead of using fixed
/// margins tuned for a single font size.
struct TabMetrics {
    label_size: f32,
    close_button_size: f32,
    margin: egui::Margin,
}

impl TabMetrics {
    fn for_font_size(font_size: f32) -> Self {
        let label_size = font_size * 0.85;
        // Approximate rendered line height for the label text; egui text
        // rows generally paint a bit taller than the nominal font size.
        let estimated_label_height = label_size * 1.25;
        let close_button_size = (label_size + 4.0).clamp(16.0, 22.0);
        let content_height = estimated_label_height.max(close_button_size);
        // Split the remaining vertical space with a 2:1 top/bottom bias,
        // matching the original hand-tuned padding, while shrinking as
        // needed so larger fonts never overflow the fixed tab height.
        let available_margin = (TITLE_TAB_HEIGHT - content_height).max(3.0);
        let top = (available_margin * (2.0 / 3.0)).clamp(2.0, 8.0);
        let bottom = (available_margin - top).clamp(1.0, 8.0);
        let horizontal = (label_size * 0.9).clamp(8.0, 14.0);
        Self {
            label_size,
            close_button_size,
            margin: egui::Margin {
                left: horizontal.round() as i8,
                right: horizontal.round() as i8,
                top: top.round() as i8,
                bottom: bottom.round() as i8,
            },
        }
    }
}

/// Paints the small application icon shown at the left of the title bar.
/// It is purely decorative, matching Notepad's non-interactive app icon.
fn show_app_icon(ui: &mut egui::Ui, app_icon_texture: &egui::TextureHandle) {
    let (rect, _response) = ui.allocate_exact_size(
        egui::vec2(TITLE_CONTROL_WIDTH, TITLE_CONTENT_HEIGHT),
        egui::Sense::hover(),
    );
    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(20.0));
    ui.painter().image(
        app_icon_texture.id(),
        icon_rect,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WindowControlKind {
    Minimize,
    Maximize,
    Restore,
    Close,
}

fn window_control_button(
    ui: &mut egui::Ui,
    kind: WindowControlKind,
    title_bar_color: egui::Color32,
) -> egui::Response {
    let is_close = kind == WindowControlKind::Close;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(WINDOW_BUTTON_WIDTH, TITLE_CONTENT_HEIGHT),
        egui::Sense::click(),
    );
    let pointer_down = response.is_pointer_button_down_on();
    let fill = if is_close && (response.hovered() || pointer_down) {
        egui::Color32::from_rgb(196, 43, 28)
    } else if pointer_down {
        ui.style().visuals.widgets.active.bg_fill
    } else if response.hovered() {
        ui.style().visuals.widgets.hovered.bg_fill
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    let stroke_color = if is_close && (response.hovered() || pointer_down) {
        egui::Color32::WHITE
    } else {
        ui.style().visuals.text_color()
    };
    let center = rect.center();

    match kind {
        WindowControlKind::Minimize => {
            let cy = center.y.round() + 0.5;
            let cx = center.x.round();
            ui.painter().line_segment(
                [egui::pos2(cx - 5.0, cy), egui::pos2(cx + 5.0, cy)],
                egui::Stroke::new(1.0, stroke_color),
            );
        }
        WindowControlKind::Maximize => {
            let box_rect = egui::Rect::from_center_size(center, egui::vec2(8.0, 8.0));
            ui.painter().rect_stroke(
                box_rect,
                0.0,
                egui::Stroke::new(1.0, stroke_color),
                egui::StrokeKind::Middle,
            );
        }
        WindowControlKind::Restore => {
            let back =
                egui::Rect::from_min_size(center + egui::vec2(-2.5, -4.5), egui::vec2(7.0, 7.0));
            ui.painter().rect_stroke(
                back,
                0.0,
                egui::Stroke::new(1.0, stroke_color),
                egui::StrokeKind::Middle,
            );
            let front =
                egui::Rect::from_min_size(center + egui::vec2(-4.5, -2.5), egui::vec2(7.0, 7.0));
            let bg_fill = if fill == egui::Color32::TRANSPARENT {
                title_bar_color
            } else {
                fill
            };
            ui.painter().rect_filled(front, 0.0, bg_fill);
            ui.painter().rect_stroke(
                front,
                0.0,
                egui::Stroke::new(1.0, stroke_color),
                egui::StrokeKind::Middle,
            );
        }
        WindowControlKind::Close => {
            let d = 4.5;
            ui.painter().line_segment(
                [
                    egui::pos2(center.x - d, center.y - d),
                    egui::pos2(center.x + d, center.y + d),
                ],
                egui::Stroke::new(1.2, stroke_color),
            );
            ui.painter().line_segment(
                [
                    egui::pos2(center.x - d, center.y + d),
                    egui::pos2(center.x + d, center.y - d),
                ],
                egui::Stroke::new(1.2, stroke_color),
            );
        }
    }
    response
}

impl GoatpadApp {
    pub(super) fn show_title_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut requested_switch = None;
        let mut requested_close = None;
        let mut requested_rename = None;
        let mut requested_new_tab = false;
        let mut finish_tab_rename = false;
        self.tab_drop_target = None;
        let tabs = self
            .session
            .open_tabs
            .iter()
            .filter_map(|id| {
                self.workspace
                    .document(*id)
                    .map(|document| (*id, document.title.clone()))
            })
            .collect::<Vec<_>>();
        egui::Panel::top("title_bar")
            .exact_size(TITLE_BAR_HEIGHT)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(self.title_bar_color)
                    .stroke(egui::Stroke::NONE)
                    .inner_margin(egui::Margin {
                        left: 10,
                        right: 0,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(TITLE_BAR_SPACING, 0.0);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::BOTTOM), |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(TITLE_CONTROL_WIDTH, TITLE_BAR_HEIGHT),
                            egui::Layout::bottom_up(egui::Align::Center),
                            |ui| {
                                show_app_icon(ui, &self.app_icon_texture);
                            },
                        );
                    });

                    if !tabs.is_empty() {
                        let fixed_width = 2.0 * TITLE_CONTROL_WIDTH
                            + 3.0 * WINDOW_BUTTON_WIDTH
                            + MIN_TITLE_DRAG_WIDTH
                            + 7.0 * TITLE_BAR_SPACING;
                        let tabs_width = (ui.available_width() - fixed_width).max(MIN_TABS_WIDTH);

                        ui.allocate_ui_with_layout(
                            egui::vec2(tabs_width, TITLE_TAB_HEIGHT),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                let rect = ui.max_rect();
                                ui.set_clip_rect(rect);

                                egui::ScrollArea::horizontal()
                                    .id_salt("title_bar_tabs")
                                    .max_width(tabs_width)
                                    .max_height(TITLE_TAB_HEIGHT)
                                    .scroll_bar_visibility(
                                        egui::containers::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                    )
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let tab_metrics =
                                                TabMetrics::for_font_size(self.theme_draft.font_size);
                                            for (id, title) in &tabs {
                                        if self.renaming_document == Some(*id) {
                                            let response = ui.add(
                                                egui::TextEdit::singleline(&mut self.rename_buffer)
                                                    .desired_width(180.0)
                                                    .hint_text("Note title"),
                                            );
                                            if self.focus_rename {
                                                response.request_focus();
                                                self.focus_rename = false;
                                            }
                                            let enter = response.has_focus()
                                                && ui.input_mut(|input| {
                                                    input.consume_key(
                                                        egui::Modifiers::NONE,
                                                        egui::Key::Enter,
                                                    )
                                                });
                                            if enter || response.lost_focus() {
                                                finish_tab_rename = true;
                                            }
                                        } else {
                                            let is_active = self.session.active_tab == Some(*id);
                                            let response = egui::Frame::new()
                                                .fill(if is_active {
                                                    self.theme_draft.background.0
                                                } else {
                                                    egui::Color32::TRANSPARENT
                                                })
                                                .corner_radius(egui::CornerRadius {
                                                    nw: 5,
                                                    ne: 5,
                                                    sw: 0,
                                                    se: 0,
                                                })
                                                .inner_margin(tab_metrics.margin)
                                                .show(ui, |ui| {
                                                    let tab_label_size = tab_metrics.label_size;
                                                    ui.with_layout(
                                                        egui::Layout::left_to_right(
                                                            egui::Align::TOP,
                                                        ),
                                                        |ui| {
                                                            let mut label =
                                                                egui::RichText::new(title)
                                                                    .size(tab_label_size)
                                                                    .strong();
                                                            if is_active {
                                                                label = label.color(
                                                                    self.theme_draft.primary.0,
                                                                );
                                                            }
                                                            let label_response = ui.label(label);
                                                            let close_response = ui
                                                                .add(
                                                                    egui::Button::new(
                                                                        egui_phosphor::regular::X,
                                                                    )
                                                                    .frame_when_inactive(false)
                                                                    .corner_radius(5)
                                                                    .min_size(egui::vec2(
                                                                        tab_metrics
                                                                            .close_button_size,
                                                                        tab_metrics
                                                                            .close_button_size,
                                                                    )),
                                                                )
                                                                .on_hover_text("Close tab");

                                                            if close_response.middle_clicked()
                                                                || close_response.clicked()
                                                            {
                                                                requested_close = Some(*id);
                                                            }

                                                            label_response
                                                        },
                                                    )
                                                    .inner
                                                })
                                                .inner
                                                .on_hover_text("Double-click to rename")
                                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                                .interact(egui::Sense::click_and_drag());

                                            if response.drag_started_by(egui::PointerButton::Primary) {
                                                self.dragged_tab = Some(*id);
                                            }
                                            if self.dragged_tab.is_some()
                                                && response.hovered()
                                                && self.dragged_tab != Some(*id)
                                            {
                                                self.tab_drop_target = Some(*id);
                                            }
                                            if response.clicked() {
                                                requested_switch = Some(*id);
                                            }
                                            if response.double_clicked() {
                                                requested_rename = Some(*id);
                                            }
                                            if is_active
                                                && self.scrolled_to_active_tab != Some(*id)
                                            {
                                                response.scroll_to_me(Some(egui::Align::Center));
                                                self.scrolled_to_active_tab = Some(*id);
                                            }
                                        }
                                            }
                                        });
                                    });
                            },
                        );
                    }

                    let desired_size = egui::vec2(TITLE_CONTENT_HEIGHT, TITLE_TAB_HEIGHT);
                    let (outer_rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

                    let button_size = egui::vec2(18.0, 18.0);
                    let button_rect = egui::Rect::from_center_size(outer_rect.center(), button_size);

                    let new_tab_response = ui
                        .put(
                            button_rect,
                            egui::Button::new(egui_phosphor::regular::PLUS)
                                .frame_when_inactive(false)
                                .corner_radius(5),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if new_tab_response.on_hover_text("New tab").clicked() {
                        requested_new_tab = true;
                    }
                    let drag_width = (ui.available_width()
                        - 3.0 * WINDOW_BUTTON_WIDTH
                        - TITLE_BAR_SPACING)
                        .max(0.0);
                    if drag_width > 0.0 {
                        let drag_response = ui.allocate_response(
                            egui::vec2(drag_width, TITLE_BAR_HEIGHT),
                            egui::Sense::drag(),
                        );
                        if drag_response.drag_started_by(egui::PointerButton::Primary) {
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }
                    }

                    let (minimize_response, maximize_response, close_response) = ui
                        .allocate_ui_with_layout(
                            egui::vec2(3.0 * WINDOW_BUTTON_WIDTH, TITLE_BAR_HEIGHT),
                            egui::Layout::left_to_right(egui::Align::TOP),
                            |ui| {
                                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

                                let minimize_response = window_control_button(
                                    ui,
                                    WindowControlKind::Minimize,
                                    self.title_bar_color,
                                );
                                let maximized =
                                    ctx.input(|input| input.viewport().maximized.unwrap_or(false));
                                let max_kind = if maximized {
                                    WindowControlKind::Restore
                                } else {
                                    WindowControlKind::Maximize
                                };
                                let maximize_response = window_control_button(
                                    ui,
                                    max_kind,
                                    self.title_bar_color,
                                );
                                let close_response = window_control_button(
                                    ui,
                                    WindowControlKind::Close,
                                    self.title_bar_color,
                                );

                                (minimize_response, maximize_response, close_response)
                            },
                        )
                        .inner;
                    if minimize_response.on_hover_text("Minimize").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
                    if maximize_response
                        .on_hover_text(if maximized { "Restore" } else { "Maximize" })
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }
                    if close_response.on_hover_text("Close").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        if self.dragged_tab.is_some() && ctx.input(|input| input.pointer.any_released()) {
            if let (Some(source), Some(target)) =
                (self.dragged_tab.take(), self.tab_drop_target.take())
            {
                if self.session.move_tab_before(source, target) {
                    self.save_session();
                }
            } else {
                self.dragged_tab = None;
            }
        }
        if let Some(id) = requested_switch {
            self.activate_tab(id);
        }
        if let Some(id) = requested_close {
            self.close_tab(id);
        }
        if let Some(id) = requested_rename {
            self.begin_rename(id);
        }
        if requested_new_tab {
            self.create_tab();
        }
        if finish_tab_rename {
            self.finish_rename();
        }
    }
}
