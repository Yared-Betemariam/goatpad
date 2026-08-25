use crate::{paths::AppPaths, persistence::atomic_write};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct WindowGeom {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct TabState {
    pub cursor_offset: usize,
    pub scroll_offset: f32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Session {
    pub active_tab: Option<Uuid>,
    pub window: Option<WindowGeom>,
    #[serde(default)]
    pub tab_state: HashMap<Uuid, TabState>,
}

impl Session {
    pub fn load(paths: &AppPaths) -> io::Result<Self> {
        let path = paths.session_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        serde_json::from_slice(&fs::read(path)?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    pub fn save(&self, paths: &AppPaths) -> io::Result<()> {
        let data = serde_json::to_vec_pretty(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        atomic_write(&paths.session_path(), &data)
    }
}
