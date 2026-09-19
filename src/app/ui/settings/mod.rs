use super::super::*;

mod keyboard;
mod themes;
mod updates;

pub(in crate::app) use updates::UpdateStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum SettingsTab {
    #[default]
    Themes,
    Keyboard,
    Updates,
}

impl SettingsTab {
    pub(in crate::app) const ALL: &[Self] = &[Self::Themes, Self::Keyboard, Self::Updates];

    pub(in crate::app) fn title(self) -> &'static str {
        match self {
            Self::Themes => "Themes",
            Self::Keyboard => "Keyboard",
            Self::Updates => "Updates",
        }
    }
}

impl GoatpadApp {
    pub(super) fn show_settings(&mut self, ctx: &egui::Context) {
        if self.settings_open {
            let mut settings_open = self.settings_open;
            egui::Window::new("Settings")
                .open(&mut settings_open)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .resizable(true)
                .default_width(480.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        for tab in SettingsTab::ALL {
                            if ui
                                .selectable_label(self.settings_tab == *tab, tab.title())
                                .clicked()
                            {
                                self.settings_tab = *tab;
                            }
                        }
                    });
                    ui.separator();

                    match self.settings_tab {
                        SettingsTab::Themes => {
                            self.render_themes_settings(ui, ctx);
                        }
                        SettingsTab::Keyboard => {
                            self.render_keyboard_settings(ui);
                        }
                        SettingsTab::Updates => {
                            self.render_updates_settings(ui);
                        }
                    }
                });
            self.settings_open = settings_open;
        }
    }
}
