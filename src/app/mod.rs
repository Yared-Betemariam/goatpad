use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::{
    collections::HashSet,
    ops::Range,
    sync::mpsc::{Receiver, Sender, TryRecvError},
    time::{Duration, Instant},
};
use uuid::Uuid;

use crate::{
    appearance::{
        self, LIGHT_THEME_ID, Theme, apply_theme, ensure_default_themes, install_fonts,
        load_themes, save_theme,
    },
    config::{
        self,
        settings::{
            CONTENT_ZOOM_STEP, DEFAULT_CONTENT_ZOOM, MAX_CONTENT_ZOOM, MIN_CONTENT_ZOOM, Settings,
        },
    },
    domain::{
        document::DocKind,
        workspace::{DropPlacement, Workspace, WorkspaceItem},
    },
    editor::{
        find as editor_find, formatting, highlighting, multicursor, spellcheck::SpellChecker,
    },
    input::hotkeys::{self, Action, Keybinding},
    services::{
        paths::AppPaths,
        persistence::{SaveRequest, SaveResult, start_writer_thread},
        session::{Session, TabState, WindowGeom},
        updates::{self as update_service, ReleaseManifest, UpdateEvent},
    },
};

mod documents;
mod editor;
mod find;
mod input;
mod persistence;
pub(crate) mod resources;
mod settings;
mod themes;
mod ui;
mod updates;
mod window;

use ui::{
    settings::{SettingsTab, UpdateStatus},
    toasts::{Toast, ToastKind},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum FindScope {
    ActiveNote,
    AllNotes,
}

#[derive(Clone)]
struct FindMatch {
    document_id: Uuid,
    range: Range<usize>,
}

struct FindState {
    scope: FindScope,
    query: String,
    focus_input: bool,
    current: Option<usize>,
}

struct FolderEditor {
    id: Option<Uuid>,
    parent_id: Option<Uuid>,
    buffer: String,
    focus: bool,
}

pub(crate) struct GoatpadApp {
    app_icon_texture: egui::TextureHandle,
    workspace: Workspace,
    paths: AppPaths,
    session: Session,
    cursor_offset: usize,
    multi_cursor_offsets: Vec<usize>,
    scroll_offset: f32,
    restore_cursor: bool,
    pending_find_scroll: Option<(Uuid, usize)>,
    writer: Sender<SaveRequest>,
    writer_results: Receiver<SaveResult>,
    last_edit: Option<Instant>,
    dirty_since: Option<Instant>,
    last_session_save: Instant,
    window_restore_target: Option<WindowGeom>,
    window_restore_pending: bool,
    window_restore_deadline: Option<Instant>,
    delete_confirmation: Option<Uuid>,
    tabs_list_open: bool,
    tabs_list_search: String,
    focus_tabs_list_search: bool,
    tabs_list_selected: Option<Uuid>,
    expanded_folders: HashSet<Uuid>,
    folder_editor: Option<FolderEditor>,
    dragged_workspace_item: Option<WorkspaceItem>,
    workspace_drop_target: Option<(WorkspaceItem, DropPlacement)>,
    dragged_tab: Option<Uuid>,
    tab_drop_target: Option<Uuid>,
    find: Option<FindState>,
    settings: Settings,
    settings_open: bool,
    settings_tab: SettingsTab,
    rebinding: Option<Action>,
    font_options: Vec<String>,
    themes: Vec<Theme>,
    theme_draft: Theme,
    title_bar_color: egui::Color32,
    editing_theme: Option<Theme>,
    editing_theme_is_new: bool,
    theme_delete_confirm: Option<String>,
    renaming_document: Option<Uuid>,
    rename_buffer: String,
    focus_rename: bool,
    scrolled_to_active_tab: Option<Uuid>,
    workspace_index_dirty: bool,
    toasts: Vec<Toast>,
    zoom: f32,
    update_status: UpdateStatus,
    update_receiver: Option<Receiver<UpdateEvent>>,
    spellchecker: SpellChecker,
    spellcheck_cache: SpellcheckCache,
    spellcheck_menu: Option<SpellcheckMenuTarget>,
}

/// Caches the most recent spell-check pass so the (comparatively expensive)
/// OS spell-checker call only re-runs when the active document or its
/// content actually changes, rather than on every frame.
#[derive(Default)]
struct SpellcheckCache {
    document_id: Option<Uuid>,
    content_hash: u64,
    ranges: Vec<Range<usize>>,
}

/// The misspelled word the user last right-clicked, kept alive while the
/// spell-check context menu is open.
#[derive(Clone)]
struct SpellcheckMenuTarget {
    document_id: Uuid,
    range: Range<usize>,
    word: String,
    suggestions: Vec<String>,
}

impl GoatpadApp {
    pub(crate) fn new(
        paths: AppPaths,
        mut session: Session,
        ctx: &egui::Context,
    ) -> std::io::Result<Self> {
        let (mut workspace, startup_warnings) = Workspace::load(paths.clone())?;
        let settings = Settings::load(&paths)?;
        let content_zoom = settings.content_zoom;
        ensure_default_themes(&paths)?;
        let themes = load_themes(&paths)?;
        let theme_draft = themes
            .iter()
            .find(|theme| theme.name == settings.theme)
            .cloned()
            .unwrap_or_else(Theme::default_dark);
        let font_options = install_fonts(ctx);
        apply_theme(ctx, &theme_draft);
        ctx.set_zoom_factor(settings.app_zoom);
        let pixels_per_point = ctx
            .input(|input| input.viewport().native_pixels_per_point)
            .unwrap_or(1.0)
            * ctx.zoom_factor();
        session.migrate_window_to_physical_pixels(pixels_per_point);
        let window_restore_target = session.window.filter(WindowGeom::is_valid);
        let note_ids = workspace
            .documents
            .iter()
            .map(|document| document.id)
            .collect::<Vec<_>>();
        session.prepare_open_tabs(&note_ids);
        let folder_ids = workspace
            .folders
            .iter()
            .map(|folder| folder.id)
            .collect::<Vec<_>>();
        session.prepare_expanded_folders(&folder_ids);
        let state = if let Some(active_id) = session.active_tab {
            workspace.set_active_by_id(active_id);
            workspace.touch_document(active_id)?;
            session
                .tab_state
                .get(&active_id)
                .copied()
                .unwrap_or_default()
        } else {
            TabState::default()
        };
        session.save(&paths)?;
        let expanded_folders = session.expanded_folders.clone();
        let (writer, writer_results) = start_writer_thread();
        let app_icon_texture = resources::load_app_icon_texture(ctx);
        let mut app = Self {
            app_icon_texture,
            workspace,
            paths,
            session,
            cursor_offset: state.cursor_offset,
            multi_cursor_offsets: Vec::new(),
            scroll_offset: state.scroll_offset,
            restore_cursor: true,
            pending_find_scroll: None,
            writer,
            writer_results,
            last_edit: None,
            dirty_since: None,
            last_session_save: Instant::now(),
            window_restore_target,
            window_restore_pending: window_restore_target.is_some(),
            window_restore_deadline: None,
            delete_confirmation: None,
            tabs_list_open: false,
            tabs_list_search: String::new(),
            focus_tabs_list_search: false,
            tabs_list_selected: None,
            expanded_folders,
            folder_editor: None,
            dragged_workspace_item: None,
            workspace_drop_target: None,
            dragged_tab: None,
            tab_drop_target: None,
            find: None,
            settings,
            settings_open: false,
            settings_tab: SettingsTab::default(),
            rebinding: None,
            font_options,
            themes,
            title_bar_color: theme_draft.title_bar_color(),
            theme_draft,
            editing_theme: None,
            editing_theme_is_new: false,
            theme_delete_confirm: None,
            renaming_document: None,
            rename_buffer: String::new(),
            focus_rename: false,
            scrolled_to_active_tab: None,
            workspace_index_dirty: false,
            toasts: startup_warnings
                .into_iter()
                .map(|message| Toast {
                    message,
                    shown_at: Instant::now(),
                    kind: ToastKind::Error,
                })
                .collect(),
            zoom: content_zoom,
            update_status: UpdateStatus::Idle,
            update_receiver: None,
            spellchecker: SpellChecker::new(),
            spellcheck_cache: SpellcheckCache::default(),
            spellcheck_menu: None,
        };
        if app.settings.auto_check_updates && !config::UPDATE_MANIFEST_URL.is_empty() {
            app.check_for_updates();
        }
        Ok(app)
    }

    fn report_error(&mut self, message: impl Into<String>) {
        self.toasts.push(Toast {
            message: message.into(),
            shown_at: Instant::now(),
            kind: ToastKind::Error,
        });
    }

    fn report_success(&mut self, message: impl Into<String>) {
        self.toasts.push(Toast {
            message: message.into(),
            shown_at: Instant::now(),
            kind: ToastKind::Success,
        });
    }
}
