use super::*;

#[derive(Clone)]
pub(in crate::app) enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available(ReleaseManifest),
    Downloading,
    Error(String),
}

impl GoatpadApp {
    pub(super) fn render_updates_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Application updates");
        ui.label(format!(
            "Installed version: {}",
            crate::services::updates::CURRENT_VERSION
        ));
        ui.add_space(6.0);
        if config::UPDATE_MANIFEST_URL.trim().is_empty() {
            ui.label("Update checking is not configured.");
            ui.label("Set UPDATE_MANIFEST_URL in src/config/mod.rs to enable releases.");
        } else {
            ui.label("Updates use the manifest configured in src/config/mod.rs.");
        }
        ui.add_space(6.0);
        if ui
            .checkbox(
                &mut self.settings.auto_check_updates,
                "Check automatically when Goatpad starts",
            )
            .changed()
        {
            if let Err(error) = self.settings.save(&self.paths) {
                self.report_error(format!("Could not save update settings: {error}"));
            }
        }
        ui.add_space(8.0);
        match self.update_status.clone() {
            UpdateStatus::Idle => {
                ui.label("Configure a secure update manifest, then check for releases.");
                if ui.button("Check for updates").clicked() {
                    self.check_for_updates();
                }
            }
            UpdateStatus::Checking => {
                ui.spinner();
                ui.label("Checking for updates…");
            }
            UpdateStatus::UpToDate => {
                ui.label("You are up to date.");
                if ui.button("Check again").clicked() {
                    self.check_for_updates();
                }
            }
            UpdateStatus::Available(release) => {
                ui.label(
                    egui::RichText::new(format!("Version {} is available", release.version))
                        .strong(),
                );
                if !release.notes.trim().is_empty() {
                    ui.label(&release.notes);
                }
                if ui.button("Download and install update").clicked() {
                    self.download_and_install_update(release);
                }
                ui.label(
                    egui::RichText::new(
                        "Goatpad will close and Windows Installer will finish the upgrade.",
                    )
                    .weak(),
                );
            }
            UpdateStatus::Downloading => {
                ui.spinner();
                ui.label("Downloading update… Goatpad will close when it is ready to install.");
            }
            UpdateStatus::Error(error) => {
                ui.colored_label(egui::Color32::from_rgb(235, 105, 105), error);
                if ui.button("Try again").clicked() {
                    self.check_for_updates();
                }
            }
        }
    }
}
