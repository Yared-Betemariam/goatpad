use super::super::*;

#[derive(Clone, Copy)]
pub(in crate::app) enum ToastKind {
    Error,
    Success,
}

pub(in crate::app) struct Toast {
    pub(in crate::app) message: String,
    pub(in crate::app) shown_at: Instant,
    pub(in crate::app) kind: ToastKind,
}

impl GoatpadApp {
    pub(super) fn show_toasts(&mut self, ctx: &egui::Context, now: Instant) {
        self.toasts
            .retain(|toast| now.duration_since(toast.shown_at) < Duration::from_secs(8));
        if !self.toasts.is_empty() {
            egui::Area::new(egui::Id::new("status_toasts"))
                .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.set_max_width(360.0);
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 8.0);

                    for toast in &self.toasts {
                        let (color, icon) = match toast.kind {
                            ToastKind::Error => (
                                egui::Color32::from_rgb(235, 105, 105),
                                egui_phosphor::regular::WARNING_CIRCLE,
                            ),
                            ToastKind::Success => (
                                egui::Color32::from_rgb(107, 193, 123),
                                egui_phosphor::regular::CHECK_CIRCLE,
                            ),
                        };
                        let frame_fill = ui.visuals().window_fill.lerp_to_gamma(color, 0.08);

                        egui::Frame::popup(ui.style())
                            .fill(frame_fill)
                            .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.75)))
                            .corner_radius(egui::CornerRadius::same(8))
                            .inner_margin(egui::Margin {
                                left: 12,
                                right: 12,
                                top: 10,
                                bottom: 10,
                            })
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(icon).size(18.0).color(color));
                                    ui.add_space(8.0);
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&toast.message)
                                                .color(ui.visuals().text_color()),
                                        )
                                        .wrap(),
                                    );
                                });
                            });
                    }
                });
        }
    }
}
