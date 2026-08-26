use crate::{
    document::{DocKind, Document},
    paths::AppPaths,
    persistence::atomic_write,
};
use serde::{Deserialize, Serialize};
use std::{fs, io};
use uuid::Uuid;

const WORKSPACE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceIndex {
    pub version: u32,
    pub tabs: Vec<TabEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TabEntry {
    pub id: Uuid,
    pub title: String,
    pub kind: DocKind,
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
        if index.tabs.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "workspace has no documents",
            ));
        }
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
                Ok(Document {
                    id: entry.id,
                    title: entry.title,
                    kind: entry.kind,
                    content,
                    dirty: false,
                })
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

    pub fn new_tab(&mut self) -> io::Result<()> {
        let document = Document::new_untitled();
        self.save_document(&document)?;
        self.documents.push(document);
        self.active = self.documents.len() - 1;
        self.save_index()
    }

    pub fn delete_tab(&mut self, id: Uuid) -> io::Result<bool> {
        if self.documents.len() == 1 {
            return Ok(false);
        }
        let Some(index) = self.documents.iter().position(|document| document.id == id) else {
            return Ok(false);
        };
        let document = self.documents.remove(index);
        let path = self.document_path(document.id, document.kind);
        if path.exists() {
            fs::remove_file(path)?;
        }
        if self.active >= self.documents.len() {
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
                    kind: document.kind,
                })
                .collect(),
        };
        let data = serde_json::to_vec_pretty(&index)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        atomic_write(&self.paths.workspace_path(), &data)
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
        assert!(paths.workspace_path().exists());
        assert!(
            workspace
                .document_path(workspace.active_document().id, DocKind::Md)
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
        let path = workspace.document_path(id, DocKind::Md);
        fs::remove_file(&path).unwrap();

        let (reloaded, warnings) = Workspace::load(paths).unwrap();

        assert_eq!(reloaded.active_document().id, id);
        assert!(reloaded.active_document().content.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(!path.exists());
        fs::remove_dir_all(directory).unwrap();
    }
}
