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

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    #[serde(default)]
    pub open_tabs: Vec<Uuid>,
    pub active_tab: Option<Uuid>,
    pub window: Option<WindowGeom>,
    #[serde(default)]
    pub tab_state: HashMap<Uuid, TabState>,
    #[serde(skip)]
    open_tabs_missing: bool,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            open_tabs: Vec::new(),
            active_tab: None,
            window: None,
            tab_state: HashMap::new(),
            open_tabs_missing: true,
        }
    }
}

impl Session {
    pub fn load(paths: &AppPaths) -> io::Result<Self> {
        let path = paths.session_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = fs::read(path)?;
        let open_tabs_missing = serde_json::from_slice::<serde_json::Value>(&data)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
            .get("open_tabs")
            .is_none();
        let mut session: Self = serde_json::from_slice(&data)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        session.open_tabs_missing = open_tabs_missing;
        Ok(session)
    }

    pub fn prepare_open_tabs(&mut self, note_ids: &[Uuid]) {
        if self.open_tabs_missing {
            self.open_tabs = note_ids.to_vec();
        } else {
            self.open_tabs.retain(|id| note_ids.contains(id));
            let mut seen = Vec::with_capacity(self.open_tabs.len());
            self.open_tabs.retain(|id| {
                if seen.contains(id) {
                    false
                } else {
                    seen.push(*id);
                    true
                }
            });
        }
        self.open_tabs_missing = false;
        if !self
            .active_tab
            .is_some_and(|id| self.open_tabs.contains(&id))
        {
            self.active_tab = self.open_tabs.first().copied();
        }
        self.tab_state.retain(|id, _| note_ids.contains(id));
    }

    pub fn open_tab(&mut self, id: Uuid) {
        if !self.open_tabs.contains(&id) {
            self.open_tabs.push(id);
        }
        self.active_tab = Some(id);
    }

    pub fn close_tab(&mut self, id: Uuid) -> bool {
        let Some(index) = self.open_tabs.iter().position(|open_id| *open_id == id) else {
            return false;
        };
        self.open_tabs.remove(index);
        if self.active_tab == Some(id) {
            self.active_tab = self
                .open_tabs
                .get(index)
                .or_else(|| {
                    index
                        .checked_sub(1)
                        .and_then(|left| self.open_tabs.get(left))
                })
                .copied();
        }
        true
    }

    pub fn cycle_tab(&mut self, forward: bool) -> Option<Uuid> {
        if self.open_tabs.len() <= 1 {
            return self.active_tab;
        }
        let current = self
            .active_tab
            .and_then(|id| self.open_tabs.iter().position(|open_id| *open_id == id))
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % self.open_tabs.len()
        } else if current == 0 {
            self.open_tabs.len() - 1
        } else {
            current - 1
        };
        self.active_tab = Some(self.open_tabs[next]);
        self.active_tab
    }

    pub fn save(&self, paths: &AppPaths) -> io::Result<()> {
        let data = serde_json::to_vec_pretty(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        atomic_write(&paths.session_path(), &data)
    }
}

#[cfg(test)]
mod tests {
    use super::Session;
    use crate::paths::AppPaths;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn old_sessions_migrate_all_notes_in_existing_order() {
        let ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let mut session = Session {
            active_tab: Some(ids[1]),
            ..Session::default()
        };

        session.prepare_open_tabs(&ids);

        assert_eq!(session.open_tabs, ids);
        assert_eq!(session.active_tab, Some(ids[1]));
    }

    #[test]
    fn migration_is_persisted_and_does_not_reopen_an_intentionally_empty_session() {
        let directory =
            std::env::temp_dir().join(format!("goatpad-session-test-{}", Uuid::new_v4()));
        let paths = AppPaths::for_test(directory.clone()).unwrap();
        let ids = [Uuid::new_v4(), Uuid::new_v4()];
        fs::write(
            paths.session_path(),
            format!(
                r#"{{"active_tab":"{}","window":null,"tab_state":{{}}}}"#,
                ids[1]
            ),
        )
        .unwrap();

        let mut session = Session::load(&paths).unwrap();
        session.prepare_open_tabs(&ids);
        assert_eq!(session.open_tabs, ids);
        session.open_tabs.clear();
        session.active_tab = None;
        session.save(&paths).unwrap();

        let mut reloaded = Session::load(&paths).unwrap();
        reloaded.prepare_open_tabs(&ids);
        assert!(reloaded.open_tabs.is_empty());
        assert_eq!(reloaded.active_tab, None);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn loading_filters_missing_and_duplicate_open_tabs() {
        let ids = [Uuid::new_v4(), Uuid::new_v4()];
        let missing = Uuid::new_v4();
        let mut session = Session {
            open_tabs: vec![ids[1], missing, ids[1], ids[0]],
            active_tab: Some(missing),
            open_tabs_missing: false,
            ..Session::default()
        };

        session.prepare_open_tabs(&ids);

        assert_eq!(session.open_tabs, vec![ids[1], ids[0]]);
        assert_eq!(session.active_tab, Some(ids[1]));
    }

    #[test]
    fn closing_active_tabs_prefers_right_then_left_then_none() {
        let ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let mut session = Session {
            open_tabs: ids.to_vec(),
            active_tab: Some(ids[1]),
            open_tabs_missing: false,
            ..Session::default()
        };

        assert!(session.close_tab(ids[1]));
        assert_eq!(session.active_tab, Some(ids[2]));
        assert!(session.close_tab(ids[2]));
        assert_eq!(session.active_tab, Some(ids[0]));
        assert!(session.close_tab(ids[0]));
        assert_eq!(session.active_tab, None);
    }

    #[test]
    fn opening_is_ordered_without_duplicates_and_cycling_wraps() {
        let ids = [Uuid::new_v4(), Uuid::new_v4()];
        let mut session = Session::default();
        session.open_tab(ids[0]);
        session.open_tab(ids[1]);
        session.open_tab(ids[0]);

        assert_eq!(session.open_tabs, ids);
        assert_eq!(session.cycle_tab(true), Some(ids[1]));
        assert_eq!(session.cycle_tab(true), Some(ids[0]));
        assert_eq!(session.cycle_tab(false), Some(ids[1]));
    }
}
