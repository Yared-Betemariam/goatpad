use crate::{
    document::{DocKind, Document, unix_timestamp_millis},
    paths::AppPaths,
    persistence::atomic_write,
};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};
use uuid::Uuid;

const WORKSPACE_VERSION: u32 = 1;
const NOTES_ARCHIVE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceIndex {
    pub version: u32,
    pub tabs: Vec<TabEntry>,
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
}

#[derive(Debug, Serialize, Deserialize)]
struct NotesArchive {
    version: u32,
    notes: Vec<ArchivedNote>,
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
}

#[derive(Debug)]
pub struct Workspace {
    pub documents: Vec<Document>,
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
                    dirty: false,
                };
                document.refresh_automatic_title();
                Ok(document)
            })
            .collect::<io::Result<Vec<_>>>()?;

        Ok((
            Self {
                documents,
                active: 0,
                paths,
            },
            warnings,
        ))
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
        let document = Document::new_untitled();
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
                })
                .collect(),
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
        if archive.version != NOTES_ARCHIVE_VERSION {
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
        let imported_count = documents.len();
        self.documents.extend(documents);
        if let Err(error) = self.save_index() {
            self.documents.truncate(first_imported);
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
        self.documents[index].kind = kind;
        if let Err(error) = self.save_index() {
            self.documents[index].kind = old_kind;
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
    use super::Workspace;
    use crate::{document::DocKind, paths::AppPaths};
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
