use crate::{
    appearance::DEFAULT_THEME_ID,
    input::hotkeys::{Action, Keybinding, default_bindings},
    services::{paths::AppPaths, persistence::atomic_write},
};
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::HashMap, fs, io};

pub const DEFAULT_CONTENT_ZOOM: f32 = 1.0;
pub const MIN_CONTENT_ZOOM: f32 = 0.5;
pub const MAX_CONTENT_ZOOM: f32 = 3.0;
pub const CONTENT_ZOOM_STEP: f32 = 0.1;
pub const DEFAULT_APP_ZOOM: f32 = 1.0;

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_theme_name")]
    pub theme: String,
    #[serde(
        default = "default_bindings",
        deserialize_with = "deserialize_keybindings"
    )]
    pub keybindings: HashMap<Action, Keybinding>,
    #[serde(default = "default_auto_check_updates")]
    pub auto_check_updates: bool,
    #[serde(default = "default_content_zoom")]
    pub content_zoom: f32,
    #[serde(default = "default_app_zoom")]
    pub app_zoom: f32,
    #[serde(default = "default_spellcheck_enabled")]
    pub spellcheck_enabled: bool,
}

fn default_auto_check_updates() -> bool {
    true
}

fn default_content_zoom() -> f32 {
    DEFAULT_CONTENT_ZOOM
}

fn default_app_zoom() -> f32 {
    DEFAULT_APP_ZOOM
}

fn default_spellcheck_enabled() -> bool {
    true
}

fn normalized_content_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() {
        zoom.clamp(MIN_CONTENT_ZOOM, MAX_CONTENT_ZOOM)
    } else {
        DEFAULT_CONTENT_ZOOM
    }
}

fn normalized_app_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() && zoom > 0.0 {
        zoom
    } else {
        DEFAULT_APP_ZOOM
    }
}

/// A bad user-defined shortcut must never make the editor unable to start.
/// Ignore only the malformed entries; the missing actions are restored to their defaults.
fn deserialize_keybindings<'de, D>(deserializer: D) -> Result<HashMap<Action, Keybinding>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = HashMap::<Action, String>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .filter_map(|(action, value)| match value.parse() {
            Ok(binding) => Some((action, binding)),
            Err(error) => {
                eprintln!("ignoring invalid {action:?} shortcut in settings: {error}");
                None
            }
        })
        .collect())
}

pub fn default_theme_name() -> String {
    DEFAULT_THEME_ID.to_owned()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: default_theme_name(),
            keybindings: default_bindings(),
            auto_check_updates: default_auto_check_updates(),
            content_zoom: default_content_zoom(),
            app_zoom: default_app_zoom(),
            spellcheck_enabled: default_spellcheck_enabled(),
        }
    }
}

impl Settings {
    pub fn reset_keybindings(&mut self) {
        self.keybindings = default_bindings();
    }

    pub fn load(paths: &AppPaths) -> io::Result<Self> {
        let path = paths.settings_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = fs::read(&path)?;
        let mut settings: Self = serde_json::from_slice(&data)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        for (action, binding) in default_bindings() {
            settings.keybindings.entry(action).or_insert(binding);
        }
        settings.content_zoom = normalized_content_zoom(settings.content_zoom);
        settings.app_zoom = normalized_app_zoom(settings.app_zoom);
        let normalized = serde_json::to_vec_pretty(&settings)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if normalized != data {
            atomic_write(&path, &normalized)?;
        }
        Ok(settings)
    }

    pub fn save(&self, paths: &AppPaths) -> io::Result<()> {
        let data = serde_json::to_vec_pretty(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        atomic_write(&paths.settings_path(), &data)
    }
}

#[cfg(test)]
mod tests {
    use super::{Settings, default_bindings};
    use crate::{input::hotkeys::Action, services::paths::AppPaths};
    use std::fs;

    #[test]
    fn invalid_saved_binding_falls_back_to_its_default() {
        let directory = std::env::temp_dir().join(format!("goatpad-test-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::for_test(directory.clone()).unwrap();
        fs::write(
            paths.settings_path(),
            r#"{"theme":"default-dark","keybindings":{"ToggleBold":"Ctrl+ControlLeft","NewTab":"Ctrl+N"}}"#,
        )
        .unwrap();

        let settings = Settings::load(&paths).unwrap();
        assert_eq!(
            settings.keybindings[&Action::ToggleBold],
            default_bindings()[&Action::ToggleBold]
        );
        assert_eq!(settings.keybindings[&Action::NewTab].to_string(), "Ctrl+N");
        assert!(
            !fs::read_to_string(paths.settings_path())
                .unwrap()
                .contains("ControlLeft")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn zoom_settings_migrate_and_normalize() {
        let directory = std::env::temp_dir().join(format!("goatpad-test-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::for_test(directory.clone()).unwrap();
        fs::write(
            paths.settings_path(),
            r#"{"theme":"default-dark","keybindings":{},"content_zoom":9.0,"app_zoom":1.25}"#,
        )
        .unwrap();

        let settings = Settings::load(&paths).unwrap();
        assert_eq!(settings.content_zoom, 3.0);
        assert_eq!(settings.app_zoom, 1.25);

        let saved = fs::read_to_string(paths.settings_path()).unwrap();
        assert!(saved.contains("\"content_zoom\": 3.0"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn resetting_keybindings_restores_all_defaults() {
        let mut settings = Settings::default();
        settings
            .keybindings
            .insert(Action::NewTab, "Ctrl+Q".parse().unwrap());

        settings.reset_keybindings();

        assert_eq!(settings.keybindings, default_bindings());
    }
}
