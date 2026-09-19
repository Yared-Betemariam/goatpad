use super::*;

impl GoatpadApp {
    pub(in crate::app) fn check_for_updates(&mut self) {
        if config::UPDATE_MANIFEST_URL.trim().is_empty() {
            self.update_status = UpdateStatus::Error(
                "Update checking is not configured in src/config/mod.rs".to_owned(),
            );
            return;
        }
        self.update_status = UpdateStatus::Checking;
        self.update_receiver = Some(update_service::check_in_background());
    }

    pub(in crate::app) fn download_and_install_update(&mut self, release: ReleaseManifest) {
        self.flush_all_now();
        self.save_session();
        self.update_status = UpdateStatus::Downloading;
        self.update_receiver = Some(update_service::download_in_background(
            release,
            self.paths.clone(),
        ));
    }

    pub(in crate::app) fn poll_update_results(&mut self, ctx: &egui::Context) {
        let Some(receiver) = &self.update_receiver else {
            return;
        };
        match receiver.try_recv() {
            Ok(UpdateEvent::Check(Ok(Some(release)))) => {
                self.update_status = UpdateStatus::Available(release.clone());
                self.report_success(format!("Goatpad {} is ready to install", release.version));
                self.update_receiver = None;
            }
            Ok(UpdateEvent::Check(Ok(None))) => {
                self.update_status = UpdateStatus::UpToDate;
                self.update_receiver = None;
            }
            Ok(UpdateEvent::Check(Err(error)) | UpdateEvent::Download(Err(error))) => {
                self.update_status = UpdateStatus::Error(error);
                self.update_receiver = None;
            }
            Ok(UpdateEvent::Download(Ok(path))) => {
                self.update_receiver = None;
                match update_service::install_after_exit(&path) {
                    Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(error) => self.update_status = UpdateStatus::Error(error),
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.update_status =
                    UpdateStatus::Error("The update task stopped unexpectedly".to_owned());
                self.update_receiver = None;
            }
        }
    }
}
