use super::*;

mod action_bar;
mod confirmations;
mod editor;
mod find_bar;
mod layout;
mod markdown_preview;
mod status_bar;
mod tabs_list;
mod title_bar;
mod window_chrome;

pub(in crate::app) mod settings;
pub(in crate::app) mod toasts;

impl eframe::App for GoatpadApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_writer_results();
        self.poll_update_results(&ctx);
        if !self.restore_window_geometry(&ctx) {
            self.update_window_geometry(&ctx);
        }
        self.dispatch_hotkeys(&ctx);

        let active_is_markdown = self
            .session
            .active_tab
            .and_then(|id| self.workspace.document(id))
            .is_some_and(|document| document.kind == DocKind::Md);
        let markdown_preview_active = active_is_markdown
            && self
                .session
                .active_tab
                .is_some_and(|id| self.session.markdown_previews.contains(&id));

        self.show_title_bar(ui);
        self.show_action_bar(ui, active_is_markdown, markdown_preview_active);
        self.show_find_bar_panel(ui);
        self.show_tabs_list(&ctx);
        self.show_status_bar(ui, active_is_markdown);
        self.show_editor(ui, &ctx);
        self.show_confirmations(&ctx);
        self.show_settings(&ctx);

        let now = Instant::now();
        self.show_toasts(&ctx, now);

        if self
            .last_edit
            .is_some_and(|last| now.duration_since(last) >= Duration::from_millis(400))
            || self
                .dirty_since
                .is_some_and(|since| now.duration_since(since) >= Duration::from_secs(2))
        {
            self.queue_active_save();
        }
        if now.duration_since(self.last_session_save) >= Duration::from_secs(1) {
            self.save_session();
        }
        if self
            .session
            .active_tab
            .and_then(|id| self.workspace.document(id))
            .is_some_and(|document| document.dirty)
        {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        self.persist_app_zoom(&ctx);
        window_chrome::show_resize_handles(&ctx);
    }

    fn on_exit(&mut self) {
        if self.renaming_document.is_some() {
            self.finish_rename();
        }
        self.flush_all_now();
        self.save_session();
        self.save_settings();
    }
}
