use super::*;

impl GoatpadApp {
    fn viewport_pixels_per_point(ctx: &egui::Context) -> Option<f32> {
        let native_pixels_per_point =
            ctx.input(|input| input.viewport().native_pixels_per_point)?;
        let pixels_per_point = native_pixels_per_point * ctx.zoom_factor();
        pixels_per_point.is_finite().then_some(pixels_per_point)
    }

    fn current_window_geometry(&self, ctx: &egui::Context, maximized: bool) -> Option<WindowGeom> {
        let pixels_per_point = Self::viewport_pixels_per_point(ctx)?;
        ctx.input(|input| {
            WindowGeom::from_viewport(
                input.viewport().inner_rect?,
                input.viewport().outer_rect?,
                pixels_per_point,
                maximized,
            )
        })
    }

    pub(in crate::app) fn restore_window_geometry(&mut self, ctx: &egui::Context) -> bool {
        let Some(target) = self.window_restore_target else {
            return false;
        };
        let now = Instant::now();

        if self.window_restore_pending {
            let Some(pixels_per_point) = Self::viewport_pixels_per_point(ctx) else {
                ctx.request_repaint_after(Duration::from_millis(50));
                return true;
            };
            let Some((size, position)) = target.viewport_values(pixels_per_point) else {
                self.window_restore_target = None;
                self.window_restore_pending = false;
                return false;
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
            if target.maximized {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            }
            self.window_restore_pending = false;
            self.window_restore_deadline = Some(now + Duration::from_secs(2));
            return true;
        }

        let restored = if target.maximized {
            ctx.input(|input| input.viewport().maximized.unwrap_or(false))
        } else {
            let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
            !maximized
                && self
                    .current_window_geometry(ctx, false)
                    .is_some_and(|current| current.approximately_matches(&target))
        };
        if restored
            || self
                .window_restore_deadline
                .is_some_and(|deadline| now >= deadline)
        {
            self.window_restore_target = None;
            self.window_restore_deadline = None;
            return false;
        }

        ctx.request_repaint_after(Duration::from_millis(50));
        true
    }

    pub(in crate::app) fn update_window_geometry(&mut self, ctx: &egui::Context) {
        let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
        if let Some(window) = self.session.window.as_mut() {
            window.maximized = maximized;
        }
        if ctx.input(|input| input.viewport().minimized.unwrap_or(false)) {
            return;
        }
        if maximized {
            if self.session.window.is_none() {
                self.session.window = self.current_window_geometry(ctx, true);
            }
            return;
        }
        if let Some(geometry) = self.current_window_geometry(ctx, false) {
            self.session.window = Some(geometry);
        }
    }
}
