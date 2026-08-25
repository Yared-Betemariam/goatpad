use crate::{
    hotkeys::{Action, Keybinding, default_bindings},
    paths::AppPaths,
    persistence::atomic_write,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io};

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_bindings")]
    pub keybindings: HashMap<Action, Keybinding>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
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
        let mut settings: Self = serde_json::from_slice(&fs::read(path)?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        for (action, binding) in default_bindings() {
            settings.keybindings.entry(action).or_insert(binding);
        }
        Ok(settings)
    }

    pub fn save(&self, paths: &AppPaths) -> io::Result<()> {
        let data = serde_json::to_vec_pretty(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        atomic_write(&paths.settings_path(), &data)
    }
}
