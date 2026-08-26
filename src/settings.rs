use crate::{
    hotkeys::{Action, Keybinding, default_bindings},
    paths::AppPaths,
    persistence::atomic_write,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::HashMap, fs, io};

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_theme_name")]
    pub theme: String,
    #[serde(
        default = "default_bindings",
        deserialize_with = "deserialize_keybindings"
    )]
    pub keybindings: HashMap<Action, Keybinding>,
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
    "default-dark".to_owned()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: default_theme_name(),
            keybindings: default_bindings(),
        }
    }
}

impl Settings {
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
    use crate::{hotkeys::Action, paths::AppPaths};
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
}
