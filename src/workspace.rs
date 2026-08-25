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
    pub fn load(paths: AppPaths) -> io::Result<Self> {
        if !paths.workspace_path().exists() {
            let workspace = Self {
                documents: vec![Document::new_untitled()],
                active: 0,
                paths,
            };
            workspace.save_document(&workspace.documents[0])?;
            workspace.save_index()?;
            return Ok(workspace);
        }

        let index: WorkspaceIndex = serde_json::from_slice(&fs::read(paths.workspace_path())?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if index.tabs.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "workspace has no documents",
            ));
        }
        let documents = index
            .tabs
            .into_iter()
            .map(|entry| {
                let content = fs::read_to_string(paths.documents_dir().join(format!(
                    "{}.{}",
                    entry.id,
                    entry.kind.extension()
                )))?;
                Ok(Document {
                    id: entry.id,
                    title: entry.title,
                    kind: entry.kind,
                    content,
                    dirty: false,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;

        Ok(Self {
            documents,
            active: 0,
            paths,
        })
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
