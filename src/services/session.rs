use crate::services::{paths::AppPaths, persistence::atomic_write};
use egui::{Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs, io,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct WindowGeom {
    /// The inner content size in physical/native pixels.
    pub width: f32,
    pub height: f32,
    /// The outer frame position in physical/native pixels.
    pub x: f32,
    pub y: f32,
    /// Whether the window was maximized when the session was saved.
    #[serde(default)]
    pub maximized: bool,
    /// Old sessions stored viewport points. New sessions store native pixels.
    #[serde(default)]
    pub physical_pixels: bool,
}

impl WindowGeom {
    const POSITION_TOLERANCE_PX: f32 = 4.0;

    pub fn from_viewport(
        inner_rect: Rect,
        outer_rect: Rect,
        pixels_per_point: f32,
        maximized: bool,
    ) -> Option<Self> {
        if !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
            return None;
        }

        let geometry = Self {
            width: inner_rect.width() * pixels_per_point,
            height: inner_rect.height() * pixels_per_point,
            x: outer_rect.left() * pixels_per_point,
            y: outer_rect.top() * pixels_per_point,
            maximized,
            physical_pixels: true,
        };
        geometry.is_valid().then_some(geometry)
    }

    pub fn is_valid(&self) -> bool {
        self.width.is_finite()
            && self.height.is_finite()
            && self.x.is_finite()
            && self.y.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }

    /// Convert a legacy viewport-point record to the new native-pixel format.
    /// The conversion is necessarily based on the current monitor because old
    /// sessions did not record the monitor scale separately.
    pub fn migrate_to_physical_pixels(&mut self, pixels_per_point: f32) {
        if self.physical_pixels || !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
            return;
        }
        self.width *= pixels_per_point;
        self.height *= pixels_per_point;
        self.x *= pixels_per_point;
        self.y *= pixels_per_point;
        self.physical_pixels = true;
    }

    pub fn viewport_values(&self, pixels_per_point: f32) -> Option<(Vec2, Pos2)> {
        if !self.is_valid() || !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
            return None;
        }

        let divisor = if self.physical_pixels {
            pixels_per_point
        } else {
            1.0
        };
        Some((
            Vec2::new(self.width / divisor, self.height / divisor),
            Pos2::new(self.x / divisor, self.y / divisor),
        ))
    }

    pub fn approximately_matches(&self, other: &Self) -> bool {
        self.physical_pixels
            && other.physical_pixels
            && (self.width - other.width).abs() <= Self::POSITION_TOLERANCE_PX
            && (self.height - other.height).abs() <= Self::POSITION_TOLERANCE_PX
            && (self.x - other.x).abs() <= Self::POSITION_TOLERANCE_PX
            && (self.y - other.y).abs() <= Self::POSITION_TOLERANCE_PX
    }
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
    #[serde(default)]
    pub markdown_previews: HashSet<Uuid>,
    #[serde(default)]
    pub expanded_folders: HashSet<Uuid>,
    #[serde(skip)]
    open_tabs_missing: bool,
    #[serde(skip)]
    expanded_folders_missing: bool,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            open_tabs: Vec::new(),
            active_tab: None,
            window: None,
            tab_state: HashMap::new(),
            markdown_previews: HashSet::new(),
            expanded_folders: HashSet::new(),
            open_tabs_missing: true,
            expanded_folders_missing: true,
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
        let value = serde_json::from_slice::<serde_json::Value>(&data)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let open_tabs_missing = value.get("open_tabs").is_none();
        let expanded_folders_missing = value.get("expanded_folders").is_none();
        let mut session: Self = serde_json::from_slice(&data)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if session.window.is_some_and(|window| !window.is_valid()) {
            session.window = None;
        }
        session.open_tabs_missing = open_tabs_missing;
        session.expanded_folders_missing = expanded_folders_missing;
        Ok(session)
    }

    pub fn migrate_window_to_physical_pixels(&mut self, pixels_per_point: f32) {
        if let Some(window) = self.window.as_mut() {
            window.migrate_to_physical_pixels(pixels_per_point);
            if !window.is_valid() {
                self.window = None;
            }
        }
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
        self.markdown_previews.retain(|id| note_ids.contains(id));
    }

    pub fn prepare_expanded_folders(&mut self, folder_ids: &[Uuid]) {
        if self.expanded_folders_missing {
            self.expanded_folders = folder_ids.iter().copied().collect();
        } else {
            self.expanded_folders.retain(|id| folder_ids.contains(id));
        }
        self.expanded_folders_missing = false;
    }

    pub fn toggle_markdown_preview(&mut self, id: Uuid) -> bool {
        if !self.markdown_previews.insert(id) {
            self.markdown_previews.remove(&id);
            false
        } else {
            true
        }
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

    pub fn move_tab_before(&mut self, id: Uuid, target: Uuid) -> bool {
        if id == target {
            return false;
        }
        let Some(source_index) = self.open_tabs.iter().position(|open_id| *open_id == id) else {
            return false;
        };
        let Some(target_index) = self.open_tabs.iter().position(|open_id| *open_id == target)
        else {
            return false;
        };
        self.open_tabs.remove(source_index);
        let insertion_index = target_index
            .saturating_sub((source_index < target_index) as usize)
            .min(self.open_tabs.len());
        self.open_tabs.insert(insertion_index, id);
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
    use super::{Session, WindowGeom};
    use crate::services::paths::AppPaths;
    use egui::{Rect, pos2, vec2};
    use std::{collections::HashSet, fs};
    use uuid::Uuid;

    #[test]
    fn window_geometry_uses_inner_size_and_native_pixels() {
        let geometry = WindowGeom::from_viewport(
            Rect::from_min_size(pos2(20.0, 30.0), vec2(800.0, 600.0)),
            Rect::from_min_size(pos2(10.0, 18.0), vec2(800.0, 600.0)),
            1.5,
            false,
        )
        .unwrap();

        assert_eq!(geometry.width, 1200.0);
        assert_eq!(geometry.height, 900.0);
        assert_eq!(geometry.x, 15.0);
        assert_eq!(geometry.y, 27.0);
        assert!(geometry.physical_pixels);

        let (size, position) = geometry.viewport_values(2.0).unwrap();
        assert_eq!(size, vec2(600.0, 450.0));
        assert_eq!(position, pos2(7.5, 13.5));
    }

    #[test]
    fn legacy_window_geometry_migrates_once() {
        let mut geometry: WindowGeom =
            serde_json::from_str(r#"{"width":800.0,"height":600.0,"x":10.0,"y":20.0}"#).unwrap();

        assert!(!geometry.physical_pixels);
        geometry.migrate_to_physical_pixels(1.25);
        assert_eq!(geometry.width, 1000.0);
        assert_eq!(geometry.height, 750.0);
        assert_eq!(geometry.x, 12.5);
        assert_eq!(geometry.y, 25.0);
        assert!(geometry.physical_pixels);

        geometry.migrate_to_physical_pixels(2.0);
        assert_eq!(geometry.width, 1000.0);
    }

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
    fn expanded_folder_state_round_trips_and_discards_missing_folders() {
        let directory =
            std::env::temp_dir().join(format!("goatpad-folder-session-test-{}", Uuid::new_v4()));
        let paths = AppPaths::for_test(directory.clone()).unwrap();
        let expanded_id = Uuid::new_v4();
        let missing_id = Uuid::new_v4();
        let mut session = Session::default();
        session.expanded_folders = HashSet::from([expanded_id, missing_id]);
        session.prepare_expanded_folders(&[expanded_id, missing_id]);
        session.save(&paths).unwrap();

        let mut reloaded = Session::load(&paths).unwrap();
        reloaded.prepare_expanded_folders(&[expanded_id]);

        assert_eq!(reloaded.expanded_folders, HashSet::from([expanded_id]));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn sessions_without_folder_state_expand_all_folders_on_migration() {
        let ids = [Uuid::new_v4(), Uuid::new_v4()];
        let mut session = Session::default();

        session.prepare_expanded_folders(&ids);

        assert_eq!(session.expanded_folders, HashSet::from(ids));
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

    #[test]
    fn moving_open_tabs_preserves_order() {
        let ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let mut session = Session {
            open_tabs: ids.to_vec(),
            active_tab: Some(ids[0]),
            open_tabs_missing: false,
            ..Session::default()
        };

        assert!(session.move_tab_before(ids[2], ids[0]));
        assert_eq!(session.open_tabs, vec![ids[2], ids[0], ids[1]]);
        assert!(!session.move_tab_before(ids[2], ids[2]));
    }

    #[test]
    fn markdown_preview_toggles_and_discards_missing_notes() {
        let id = Uuid::new_v4();
        let mut session = Session::default();

        assert!(session.toggle_markdown_preview(id));
        assert!(session.markdown_previews.contains(&id));
        assert!(!session.toggle_markdown_preview(id));
        assert!(session.markdown_previews.is_empty());

        session.toggle_markdown_preview(id);
        session.prepare_open_tabs(&[]);
        assert!(session.markdown_previews.is_empty());
    }
}
