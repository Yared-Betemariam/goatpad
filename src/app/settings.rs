use super::*;

impl GoatpadApp {
    pub(in crate::app) fn save_settings(&mut self) {
        if let Err(error) = self.settings.save(&self.paths) {
            self.report_error(format!("Could not save settings: {error}"));
        }
    }

    pub(in crate::app) fn set_content_zoom(&mut self, zoom: f32) {
        let zoom = zoom.clamp(MIN_CONTENT_ZOOM, MAX_CONTENT_ZOOM);
        if (self.zoom - zoom).abs() > f32::EPSILON {
            self.zoom = zoom;
            self.settings.content_zoom = zoom;
            self.save_settings();
        }
    }

    pub(in crate::app) fn persist_app_zoom(&mut self, ctx: &egui::Context) {
        let zoom = ctx.zoom_factor();
        if zoom.is_finite() && zoom > 0.0 && (self.settings.app_zoom - zoom).abs() > f32::EPSILON {
            self.settings.app_zoom = zoom;
            self.save_settings();
        }
    }

    pub(in crate::app) fn reset_keyboard_shortcuts(&mut self) {
        self.settings.reset_keybindings();
        self.rebinding = None;
        match self.settings.save(&self.paths) {
            Ok(()) => self.report_success("Keyboard shortcuts reset to defaults"),
            Err(error) => {
                self.report_error(format!("Could not save reset keyboard shortcuts: {error}"))
            }
        }
    }
}
