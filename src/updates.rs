use crate::{paths::AppPaths, persistence::atomic_write};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseManifest {
    pub version: String,
    pub msi_url: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug)]
pub enum UpdateEvent {
    Check(Result<Option<ReleaseManifest>, String>),
    Download(Result<PathBuf, String>),
}

pub fn check_in_background() -> Receiver<UpdateEvent> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(UpdateEvent::Check(check_for_update(
            crate::config::UPDATE_MANIFEST_URL,
        )));
    });
    receiver
}

pub fn download_in_background(release: ReleaseManifest, paths: AppPaths) -> Receiver<UpdateEvent> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(UpdateEvent::Download(download_release(&release, &paths)));
    });
    receiver
}

fn check_for_update(manifest_url: &str) -> Result<Option<ReleaseManifest>, String> {
    require_https(manifest_url, "update manifest")?;
    let body = fetch(manifest_url)?;
    let release: ReleaseManifest = serde_json::from_slice(&body)
        .map_err(|error| format!("The update manifest is invalid: {error}"))?;
    require_https(&release.msi_url, "MSI download")?;
    let latest = Version::parse(&release.version)
        .map_err(|error| format!("The manifest version is invalid: {error}"))?;
    let current = Version::parse(CURRENT_VERSION).expect("Cargo package version must be semver");
    Ok((latest > current).then_some(release))
}

fn download_release(release: &ReleaseManifest, paths: &AppPaths) -> Result<PathBuf, String> {
    let version = Version::parse(&release.version)
        .map_err(|error| format!("The update version is invalid: {error}"))?;
    let data = fetch(&release.msi_url)?;
    if data.is_empty() {
        return Err("The update download was empty".to_owned());
    }
    if let Some(expected) = &release.sha256 {
        let actual = format!("{:x}", Sha256::digest(&data));
        if !actual.eq_ignore_ascii_case(expected.trim()) {
            return Err(
                "The downloaded update did not pass its SHA-256 integrity check".to_owned(),
            );
        }
    }
    fs::create_dir_all(paths.updates_dir())
        .map_err(|error| format!("Could not prepare the update folder: {error}"))?;
    let path = paths
        .updates_dir()
        .join(format!("Goatpad-{version}-x64.msi"));
    atomic_write(&path, &data).map_err(|error| format!("Could not save the update: {error}"))?;
    Ok(path)
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::get(url)
        .call()
        .map_err(|error| format!("Could not contact the update server: {error}"))?;
    response
        .into_body()
        .read_to_vec()
        .map_err(|error| format!("Could not read the update server response: {error}"))
}

fn require_https(url: &str, label: &str) -> Result<(), String> {
    if url.trim().starts_with("https://") {
        Ok(())
    } else {
        Err(format!("For safety, the {label} URL must use HTTPS"))
    }
}

/// Starts an elevated MSI upgrade after Goatpad exits, avoiding a locked executable.
pub fn install_after_exit(msi_path: &PathBuf) -> Result<(), String> {
    let path = msi_path.display().to_string().replace('\'', "''");
    let script = format!(
        "Wait-Process -Id {}; Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/i', '{}', '/passive', '/norestart') -Verb RunAs",
        std::process::id(),
        path
    );
    Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Could not start the Windows installer: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check_for_update, require_https};

    #[test]
    fn rejects_unencrypted_update_urls() {
        assert!(require_https("http://example.com/update.json", "manifest").is_err());
        assert!(require_https("https://example.com/update.json", "manifest").is_ok());
    }

    #[test]
    fn empty_url_is_not_a_valid_update_source() {
        assert!(check_for_update("").is_err());
    }
}
