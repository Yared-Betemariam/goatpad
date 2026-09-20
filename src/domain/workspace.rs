use crate::{
    domain::document::{DocKind, Document, unix_timestamp_millis},
    services::{paths::AppPaths, persistence::atomic_write},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::Path,
};
use uuid::Uuid;

const WORKSPACE_VERSION: u32 = 2;
const NOTES_ARCHIVE_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceItem {
    Document(Uuid),
    Folder(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPlacement {
    Into,
    Before,
    After,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub order: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceIndex {
    pub version: u32,
    pub tabs: Vec<TabEntry>,
    #[serde(default)]
    pub folders: Vec<Folder>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TabEntry {
    pub id: Uuid,
    pub title: String,
    #[serde(default)]
    pub title_is_custom: bool,
    pub kind: DocKind,
    #[serde(default)]
    pub last_opened_at: u64,
    #[serde(default)]
    pub folder_id: Option<Uuid>,
    #[serde(default)]
    pub order: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct NotesArchive {
    version: u32,
    notes: Vec<ArchivedNote>,
    #[serde(default)]
    folders: Vec<ArchivedFolder>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArchivedFolder {
    id: Uuid,
    name: String,
    #[serde(default)]
    parent_id: Option<Uuid>,
    #[serde(default)]
    order: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArchivedNote {
    title: String,
    #[serde(default)]
    title_is_custom: bool,
    kind: DocKind,
    content: String,
    #[serde(default)]
    last_opened_at: u64,
    #[serde(default)]
    folder_id: Option<Uuid>,
    #[serde(default)]
    order: u64,
}

#[derive(Debug)]
pub struct Workspace {
    pub documents: Vec<Document>,
    pub folders: Vec<Folder>,
    pub active: usize,
    paths: AppPaths,
}

impl Workspace {
    /// Loads all workspace documents. Unreadable content is deliberately not
    /// overwritten; the caller receives a warning to show in the editor.
    pub fn load(paths: AppPaths) -> io::Result<(Self, Vec<String>)> {
        if !paths.workspace_path().exists() {
            let workspace = Self {
                documents: vec![Document::new_untitled()],
                folders: Vec::new(),
                active: 0,
                paths,
            };
            workspace.save_document(&workspace.documents[0])?;
            workspace.save_index()?;
            return Ok((workspace, Vec::new()));
        }

        let index: WorkspaceIndex = serde_json::from_slice(&fs::read(paths.workspace_path())?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let mut warnings = Vec::new();
        let documents = index
            .tabs
            .into_iter()
            .map(|entry| {
                let path =
                    paths
                        .documents_dir()
                        .join(format!("{}.{}", entry.id, entry.kind.extension()));
                let content = match fs::read_to_string(&path) {
                    Ok(content) => content,
                    Err(error) => {
                        warnings.push(format!(
                            "Could not read '{}'; its file was left untouched: {error}",
                            entry.title
                        ));
                        String::new()
                    }
                };
                let mut document = Document {
                    id: entry.id,
                    title: entry.title,
                    title_is_custom: entry.title_is_custom,
                    kind: entry.kind,
                    content,
                    last_opened_at: entry.last_opened_at,
                    folder_id: entry.folder_id,
                    order: entry.order,
                    dirty: false,
                };
                document.refresh_automatic_title();
                Ok(document)
            })
            .collect::<io::Result<Vec<_>>>()?;

        let mut workspace = Self {
            documents,
            folders: index.folders,
            active: 0,
            paths,
        };
        workspace.normalize_orders();
        Ok((workspace, warnings))
    }

    fn normalize_orders(&mut self) {
        let folder_ids = self
            .folders
            .iter()
            .map(|folder| folder.id)
            .collect::<HashSet<_>>();
        for folder in &mut self.folders {
            if folder
                .parent_id
                .is_some_and(|parent| !folder_ids.contains(&parent))
            {
                folder.parent_id = None;
            }
        }
        let mut parents = vec![None];
        parents.extend(self.folders.iter().map(|folder| Some(folder.id)));
        for parent in parents {
            let items = self.children_of(parent);
            for (order, item) in items.into_iter().enumerate() {
                self.set_item_order(item, order as u64);
            }
        }
    }

    pub fn children_of(&self, parent_id: Option<Uuid>) -> Vec<WorkspaceItem> {
        let mut items = self
            .folders
            .iter()
            .filter(|folder| folder.parent_id == parent_id)
            .map(|folder| (folder.order, WorkspaceItem::Folder(folder.id)))
            .chain(
                self.documents
                    .iter()
                    .filter(|document| document.folder_id == parent_id)
                    .map(|document| (document.order, WorkspaceItem::Document(document.id))),
            )
            .collect::<Vec<_>>();
        items.sort_by_key(|(order, _)| *order);
        items.into_iter().map(|(_, item)| item).collect()
    }

    pub fn folder(&self, id: Uuid) -> Option<&Folder> {
        self.folders.iter().find(|folder| folder.id == id)
    }

    fn item_exists(&self, item: WorkspaceItem) -> bool {
        match item {
            WorkspaceItem::Document(id) => self.document(id).is_some(),
            WorkspaceItem::Folder(id) => self.folder(id).is_some(),
        }
    }

    fn item_parent(&self, item: WorkspaceItem) -> Option<Uuid> {
        match item {
            WorkspaceItem::Document(id) => {
                self.document(id).and_then(|document| document.folder_id)
            }
            WorkspaceItem::Folder(id) => self.folder(id).and_then(|folder| folder.parent_id),
        }
    }

    fn set_item_parent(&mut self, item: WorkspaceItem, parent_id: Option<Uuid>) {
        match item {
            WorkspaceItem::Document(id) => {
                if let Some(document) = self.documents.iter_mut().find(|document| document.id == id)
                {
                    document.folder_id = parent_id;
                }
            }
            WorkspaceItem::Folder(id) => {
                if let Some(folder) = self.folders.iter_mut().find(|folder| folder.id == id) {
                    folder.parent_id = parent_id;
                }
            }
        }
    }

    fn set_item_order(&mut self, item: WorkspaceItem, order: u64) {
        match item {
            WorkspaceItem::Document(id) => {
                if let Some(document) = self.documents.iter_mut().find(|document| document.id == id)
                {
                    document.order = order;
                }
            }
            WorkspaceItem::Folder(id) => {
                if let Some(folder) = self.folders.iter_mut().find(|folder| folder.id == id) {
                    folder.order = order;
                }
            }
        }
    }

    fn next_order(&self, parent_id: Option<Uuid>) -> u64 {
        self.children_of(parent_id).len() as u64
    }

    fn folder_contains(&self, folder_id: Uuid, possible_descendant: Uuid) -> bool {
        let mut current = Some(possible_descendant);
        while let Some(id) = current {
            if id == folder_id {
                return true;
            }
            current = self.folder(id).and_then(|folder| folder.parent_id);
        }
        false
    }

    pub fn create_folder(&mut self, parent_id: Option<Uuid>, name: &str) -> io::Result<Uuid> {
        let name = name.trim();
        if name.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "folder name cannot be empty",
            ));
        }
        if parent_id.is_some_and(|id| self.folder(id).is_none()) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "parent folder not found",
            ));
        }
        let folder = Folder {
            id: Uuid::new_v4(),
            name: name.to_owned(),
            parent_id,
            order: self.next_order(parent_id),
        };
        let id = folder.id;
        self.folders.push(folder);
        if let Err(error) = self.save_index() {
            self.folders.pop();
            return Err(error);
        }
        Ok(id)
    }

    pub fn rename_folder(&mut self, id: Uuid, name: &str) -> io::Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "folder name cannot be empty",
            ));
        }
        let folder = self
            .folders
            .iter_mut()
            .find(|folder| folder.id == id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "folder not found"))?;
        let old_name = folder.name.clone();
        folder.name = name.to_owned();
        if let Err(error) = self.save_index() {
            if let Some(folder) = self.folders.iter_mut().find(|folder| folder.id == id) {
                folder.name = old_name;
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn delete_folder(&mut self, id: Uuid) -> io::Result<bool> {
        let Some(folder_index) = self.folders.iter().position(|folder| folder.id == id) else {
            return Ok(false);
        };
        let old_documents = self.documents.clone();
        let old_folders = self.folders.clone();
        let parent_id = self.folders[folder_index].parent_id;
        let children = self.children_of(Some(id));
        let siblings = self.children_of(parent_id);
        let insertion_index = siblings
            .iter()
            .position(|item| *item == WorkspaceItem::Folder(id))
            .unwrap_or(siblings.len());
        for child in &children {
            self.set_item_parent(*child, parent_id);
        }
        self.folders.remove(folder_index);
        let mut reordered = siblings;
        reordered.retain(|item| *item != WorkspaceItem::Folder(id));
        reordered.splice(insertion_index..insertion_index, children);
        for (order, item) in reordered.into_iter().enumerate() {
            self.set_item_order(item, order as u64);
        }
        if let Err(error) = self.save_index() {
            self.documents = old_documents;
            self.folders = old_folders;
            return Err(error);
        }
        Ok(true)
    }

    pub fn move_item(
        &mut self,
        item: WorkspaceItem,
        target: WorkspaceItem,
        placement: DropPlacement,
    ) -> io::Result<bool> {
        if item == target || !self.item_exists(item) || !self.item_exists(target) {
            return Ok(false);
        }
        if matches!(placement, DropPlacement::Into) && !matches!(target, WorkspaceItem::Folder(_)) {
            return Ok(false);
        }
        if let WorkspaceItem::Folder(folder_id) = item {
            if let WorkspaceItem::Folder(target_folder_id) = target {
                if self.folder_contains(folder_id, target_folder_id) {
                    return Ok(false);
                }
            }
        }
        let old_documents = self.documents.clone();
        let old_folders = self.folders.clone();
        let destination_parent = match placement {
            DropPlacement::Into => match target {
                WorkspaceItem::Folder(id) => Some(id),
                WorkspaceItem::Document(_) => return Ok(false),
            },
            DropPlacement::Before | DropPlacement::After => self.item_parent(target),
        };
        let mut siblings = self.children_of(destination_parent);
        siblings.retain(|sibling| *sibling != item);
        let insertion_index = match placement {
            DropPlacement::Into => siblings.len(),
            DropPlacement::Before | DropPlacement::After => {
                let Some(target_index) = siblings.iter().position(|sibling| *sibling == target)
                else {
                    return Ok(false);
                };
                if placement == DropPlacement::Before {
                    target_index
                } else {
                    target_index + 1
                }
            }
        };
        self.set_item_parent(item, destination_parent);
        siblings.insert(insertion_index.min(siblings.len()), item);
        for (order, sibling) in siblings.into_iter().enumerate() {
            self.set_item_order(sibling, order as u64);
        }
        self.normalize_orders();
        if let Err(error) = self.save_index() {
            self.documents = old_documents;
            self.folders = old_folders;
            return Err(error);
        }
        Ok(true)
    }

    pub fn active_document(&self) -> &Document {
        &self.documents[self.active]
    }

    pub fn active_document_mut(&mut self) -> &mut Document {
        &mut self.documents[self.active]
    }

    pub fn set_active_by_id(&mut self, id: Uuid) -> bool {
        if let Some(index) = self.documents.iter().position(|document| document.id == id) {
            self.active = index;
            true
        } else {
            false
        }
    }

    pub fn document(&self, id: Uuid) -> Option<&Document> {
        self.documents.iter().find(|document| document.id == id)
    }

    pub fn touch_document(&mut self, id: Uuid) -> io::Result<()> {
        let document = self
            .documents
            .iter_mut()
            .find(|document| document.id == id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "document not found"))?;
        document.last_opened_at = unix_timestamp_millis();
        self.save_index()
    }

    pub fn new_tab(&mut self) -> io::Result<Uuid> {
        let mut document = Document::new_untitled();
        document.order = self.next_order(None);
        let id = document.id;
        self.save_document(&document)?;
        self.documents.push(document);
        self.active = self.documents.len() - 1;
        self.save_index()?;
        Ok(id)
    }

    pub fn delete_note(&mut self, id: Uuid) -> io::Result<bool> {
        let Some(index) = self.documents.iter().position(|document| document.id == id) else {
            return Ok(false);
        };
        let document = self.documents.remove(index);
        let path = self.document_path(document.id, document.kind);
        if path.exists() {
            fs::remove_file(path)?;
        }
        if self.documents.is_empty() {
            self.active = 0;
        } else if self.active >= self.documents.len() {
            self.active = self.documents.len() - 1;
        } else if index < self.active {
            self.active -= 1;
        }
        self.normalize_orders();
        self.save_index()?;
        Ok(true)
    }

    pub fn save_index(&self) -> io::Result<()> {
        let index = WorkspaceIndex {
            version: WORKSPACE_VERSION,
            tabs: self
                .documents
                .iter()
                .map(|document| TabEntry {
                    id: document.id,
                    title: document.title.clone(),
                    title_is_custom: document.title_is_custom,
                    kind: document.kind,
                    last_opened_at: document.last_opened_at,
                    folder_id: document.folder_id,
                    order: document.order,
                })
                .collect(),
            folders: self.folders.clone(),
        };
        let data = serde_json::to_vec_pretty(&index)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        atomic_write(&self.paths.workspace_path(), &data)
    }

    pub fn rename_document(&mut self, id: Uuid, title: &str) -> io::Result<()> {
        let index = self
            .documents
            .iter()
            .position(|document| document.id == id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "document not found"))?;
        let old_title = self.documents[index].title.clone();
        let old_title_is_custom = self.documents[index].title_is_custom;
        self.documents[index].rename(title);
        if let Err(error) = self.save_index() {
            self.documents[index].title = old_title;
            self.documents[index].title_is_custom = old_title_is_custom;
            return Err(error);
        }
        Ok(())
    }

    pub fn export_notes(&self, path: &Path) -> io::Result<()> {
        let archive = NotesArchive {
            version: NOTES_ARCHIVE_VERSION,
            notes: self
                .documents
                .iter()
                .map(|document| ArchivedNote {
                    title: document.title.clone(),
                    title_is_custom: document.title_is_custom,
                    kind: document.kind,
                    content: document.content.clone(),
                    last_opened_at: document.last_opened_at,
                    folder_id: document.folder_id,
                    order: document.order,
                })
                .collect(),
            folders: self
                .folders
                .iter()
                .map(|folder| ArchivedFolder {
                    id: folder.id,
                    name: folder.name.clone(),
                    parent_id: folder.parent_id,
                    order: folder.order,
                })
                .collect(),
        };
        let data = serde_json::to_vec_pretty(&archive)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        atomic_write(path, &data)
    }

    pub fn import_notes(&mut self, path: &Path) -> io::Result<usize> {
        let archive: NotesArchive = serde_json::from_slice(&fs::read(path)?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if !matches!(archive.version, 1 | NOTES_ARCHIVE_VERSION) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported notes archive version {}", archive.version),
            ));
        }
        if archive.notes.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "notes archive is empty",
            ));
        }

        let folder_id_map = archive
            .folders
            .iter()
            .map(|folder| (folder.id, Uuid::new_v4()))
            .collect::<HashMap<_, _>>();
        let imported_folders = archive
            .folders
            .iter()
            .map(|folder| Folder {
                id: folder_id_map[&folder.id],
                name: if folder.name.trim().is_empty() {
                    "Imported folder".to_owned()
                } else {
                    folder.name.trim().to_owned()
                },
                parent_id: folder
                    .parent_id
                    .and_then(|parent| folder_id_map.get(&parent).copied()),
                order: folder.order,
            })
            .collect::<Vec<_>>();
        let documents = archive
            .notes
            .into_iter()
            .map(|note| {
                let title_is_custom = note.title_is_custom && !note.title.trim().is_empty();
                let mut document = Document {
                    id: Uuid::new_v4(),
                    title: note.title.trim().to_owned(),
                    title_is_custom,
                    kind: note.kind,
                    content: note.content,
                    last_opened_at: note.last_opened_at,
                    folder_id: note
                        .folder_id
                        .and_then(|folder| folder_id_map.get(&folder).copied()),
                    order: note.order,
                    dirty: false,
                };
                document.refresh_automatic_title();
                document
            })
            .collect::<Vec<_>>();

        let mut written_paths = Vec::with_capacity(documents.len());
        for document in &documents {
            if let Err(error) = self.save_document(document) {
                for path in written_paths {
                    let _ = fs::remove_file(path);
                }
                return Err(error);
            }
            written_paths.push(self.document_path(document.id, document.kind));
        }

        let first_imported = self.documents.len();
        let first_imported_folder = self.folders.len();
        let imported_count = documents.len();
        self.folders.extend(imported_folders);
        self.documents.extend(documents);
        self.normalize_orders();
        if let Err(error) = self.save_index() {
            self.documents.truncate(first_imported);
            self.folders.truncate(first_imported_folder);
            for path in written_paths {
                let _ = fs::remove_file(path);
            }
            return Err(error);
        }
        Ok(imported_count)
    }

    pub fn set_document_kind(&mut self, id: Uuid, kind: DocKind) -> io::Result<()> {
        let index = self
            .documents
            .iter()
            .position(|document| document.id == id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "document not found"))?;
        let old_kind = self.documents[index].kind;
        if old_kind == kind {
            return Ok(());
        }

        let old_path = self.document_path(id, old_kind);
        let new_path = self.document_path(id, kind);
        fs::rename(&old_path, &new_path)?;
        let old_title = self.documents[index].title.clone();
        self.documents[index].kind = kind;
        self.documents[index].refresh_automatic_title();
        if let Err(error) = self.save_index() {
            self.documents[index].kind = old_kind;
            self.documents[index].title = old_title;
            let _ = fs::rename(&new_path, &old_path);
            return Err(error);
        }
        Ok(())
    }

    pub fn document_path(&self, id: Uuid, kind: DocKind) -> std::path::PathBuf {
        self.paths
            .documents_dir()
            .join(format!("{}.{}", id, kind.extension()))
    }

    pub fn save_document(&self, document: &Document) -> io::Result<()> {
        atomic_write(
            &self.document_path(document.id, document.kind),
            document.content.as_bytes(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{DropPlacement, Workspace, WorkspaceItem};
    use crate::{domain::document::DocKind, services::paths::AppPaths};
    use std::fs;

    #[test]
    fn first_launch_creates_a_persisted_untitled_tab() {
        let directory =
            std::env::temp_dir().join(format!("goatpad-workspace-test-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::for_test(directory.clone()).unwrap();

        let (workspace, warnings) = Workspace::load(paths.clone()).unwrap();

        assert!(warnings.is_empty());
        assert_eq!(workspace.documents.len(), 1);
        assert_eq!(workspace.active_document().title, "Untitled");
        assert_eq!(workspace.active_document().kind, DocKind::Txt);
        assert!(paths.workspace_path().exists());
        assert!(
            workspace
                .document_path(workspace.active_document().id, DocKind::Txt)
                .exists()
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn changing_kind_renames_the_content_file_and_survives_reload() {
        let directory =
            std::env::temp_dir().join(format!("goatpad-workspace-test-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::for_test(directory.clone()).unwrap();
        let (mut workspace, warnings) = Workspace::load(paths.clone()).unwrap();
        assert!(warnings.is_empty());
        let id = workspace.active_document().id;
        workspace.active_document_mut().content = "Keep this content".to_owned();
        workspace.active_document_mut().dirty = true;
        workspace
            .save_document(workspace.active_document())
            .unwrap();

        workspace.set_document_kind(id, DocKind::Txt).unwrap();

        assert!(!workspace.document_path(id, DocKind::Md).exists());
        assert_eq!(
            fs::read_to_string(workspace.document_path(id, DocKind::Txt)).unwrap(),
            "Keep this content"
        );
        let (reloaded, warnings) = Workspace::load(paths).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(reloaded.active_document().kind, DocKind::Txt);
        assert_eq!(reloaded.active_document().content, "Keep this content");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn missing_content_file_is_reported_without_recreating_it() {
        let directory =
            std::env::temp_dir().join(format!("goatpad-workspace-test-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::for_test(directory.clone()).unwrap();
        let (workspace, warnings) = Workspace::load(paths.clone()).unwrap();
        assert!(warnings.is_empty());
        let id = workspace.active_document().id;
        let path = workspace.document_path(id, DocKind::Txt);
        fs::remove_file(&path).unwrap();

        let (reloaded, warnings) = Workspace::load(paths).unwrap();

        assert_eq!(reloaded.active_document().id, id);
        assert!(reloaded.active_document().content.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(!path.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn renamed_document_title_survives_reload() {
        let directory =
            std::env::temp_dir().join(format!("goatpad-workspace-test-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::for_test(directory.clone()).unwrap();
        let (mut workspace, _) = Workspace::load(paths.clone()).unwrap();
        let id = workspace.active_document().id;

        workspace.rename_document(id, "Project ideas").unwrap();
        let (reloaded, warnings) = Workspace::load(paths).unwrap();

        assert!(warnings.is_empty());
        assert_eq!(reloaded.active_document().title, "Project ideas");
        assert!(reloaded.active_document().title_is_custom);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn exported_notes_can_be_imported_with_fresh_ids() {
        let source_directory =
            std::env::temp_dir().join(format!("goatpad-export-test-{}", uuid::Uuid::new_v4()));
        let target_directory =
            std::env::temp_dir().join(format!("goatpad-import-test-{}", uuid::Uuid::new_v4()));
        let source_paths = AppPaths::for_test(source_directory.clone()).unwrap();
        let target_paths = AppPaths::for_test(target_directory.clone()).unwrap();
        let archive_path = source_directory.join("notes.goatpad.json");
        let (mut source, _) = Workspace::load(source_paths).unwrap();
        source.active_document_mut().content = "Automatic name\nBody".to_owned();
        source.active_document_mut().refresh_automatic_title();
        source.save_document(source.active_document()).unwrap();
        source.new_tab().unwrap();
        let second_id = source.active_document().id;
        source.active_document_mut().content = "Other content".to_owned();
        source.rename_document(second_id, "Custom name").unwrap();
        source.save_document(source.active_document()).unwrap();
        let source_ids = source
            .documents
            .iter()
            .map(|document| document.id)
            .collect::<Vec<_>>();
        source.export_notes(&archive_path).unwrap();

        let (mut target, _) = Workspace::load(target_paths.clone()).unwrap();
        let target_active_id = target.active_document().id;
        assert_eq!(target.import_notes(&archive_path).unwrap(), 2);

        assert_eq!(target.active_document().id, target_active_id);
        assert_eq!(target.documents.len(), 3);
        assert_eq!(target.documents[1].title, "Automatic name");
        assert!(!target.documents[1].title_is_custom);
        assert_eq!(target.documents[1].content, "Automatic name\nBody");
        assert_eq!(target.documents[2].title, "Custom name");
        assert!(target.documents[2].title_is_custom);
        assert!(
            target.documents[1..]
                .iter()
                .all(|document| !source_ids.contains(&document.id))
        );
        let (reloaded, warnings) = Workspace::load(target_paths).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(reloaded.documents.len(), 3);
        assert_eq!(reloaded.documents[2].content, "Other content");

        fs::remove_dir_all(source_directory).unwrap();
        fs::remove_dir_all(target_directory).unwrap();
    }

    #[test]
    fn folders_support_nesting_reordering_and_persistence() {
        let directory =
            std::env::temp_dir().join(format!("goatpad-folder-test-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::for_test(directory.clone()).unwrap();
        let (mut workspace, _) = Workspace::load(paths.clone()).unwrap();
        let first_id = workspace.documents[0].id;
        let second_id = workspace.new_tab().unwrap();
        let third_id = workspace.new_tab().unwrap();
        let projects_id = workspace.create_folder(None, "Projects").unwrap();
        let active_id = workspace
            .create_folder(Some(projects_id), "Active")
            .unwrap();

        assert!(
            workspace
                .move_item(
                    WorkspaceItem::Document(first_id),
                    WorkspaceItem::Folder(projects_id),
                    DropPlacement::Into,
                )
                .unwrap()
        );
        assert!(
            workspace
                .move_item(
                    WorkspaceItem::Document(second_id),
                    WorkspaceItem::Folder(active_id),
                    DropPlacement::Into,
                )
                .unwrap()
        );
        workspace
            .move_item(
                WorkspaceItem::Document(third_id),
                WorkspaceItem::Folder(active_id),
                DropPlacement::Into,
            )
            .unwrap();
        assert!(
            workspace
                .move_item(
                    WorkspaceItem::Document(third_id),
                    WorkspaceItem::Document(second_id),
                    DropPlacement::Before,
                )
                .unwrap()
        );
        assert_eq!(
            workspace.children_of(Some(active_id)),
            vec![
                WorkspaceItem::Document(third_id),
                WorkspaceItem::Document(second_id)
            ]
        );

        let (reloaded, warnings) = Workspace::load(paths.clone()).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(
            reloaded.document(first_id).unwrap().folder_id,
            Some(projects_id)
        );
        assert_eq!(
            reloaded.document(second_id).unwrap().folder_id,
            Some(active_id)
        );
        assert_eq!(
            reloaded.folder(active_id).unwrap().parent_id,
            Some(projects_id)
        );

        assert!(reloaded.folder(projects_id).is_some());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn deleting_a_folder_moves_direct_contents_to_its_parent() {
        let directory = std::env::temp_dir().join(format!(
            "goatpad-folder-delete-test-{}",
            uuid::Uuid::new_v4()
        ));
        let paths = AppPaths::for_test(directory.clone()).unwrap();
        let (mut workspace, _) = Workspace::load(paths.clone()).unwrap();
        let note_id = workspace.documents[0].id;
        let folder_id = workspace.create_folder(None, "Temporary").unwrap();
        workspace
            .move_item(
                WorkspaceItem::Document(note_id),
                WorkspaceItem::Folder(folder_id),
                DropPlacement::Into,
            )
            .unwrap();

        assert!(workspace.delete_folder(folder_id).unwrap());
        assert_eq!(workspace.document(note_id).unwrap().folder_id, None);
        assert!(workspace.folder(folder_id).is_none());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn deleting_the_last_note_leaves_a_valid_empty_workspace() {
        let directory =
            std::env::temp_dir().join(format!("goatpad-delete-test-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::for_test(directory.clone()).unwrap();
        let (mut workspace, _) = Workspace::load(paths.clone()).unwrap();
        let id = workspace.active_document().id;

        assert!(workspace.delete_note(id).unwrap());
        assert!(workspace.documents.is_empty());
        let (reloaded, warnings) = Workspace::load(paths).unwrap();
        assert!(warnings.is_empty());
        assert!(reloaded.documents.is_empty());

        fs::remove_dir_all(directory).unwrap();
    }
}
