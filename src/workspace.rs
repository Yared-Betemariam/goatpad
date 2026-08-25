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
    pub document: Document,
    paths: AppPaths,
}

impl Workspace {
    pub fn load(paths: AppPaths) -> io::Result<Self> {
        if !paths.workspace_path().exists() {
            let workspace = Self {
                document: Document::new_untitled(),
                paths,
            };
            workspace.save_document(&workspace.document)?;
            workspace.save_index()?;
            return Ok(workspace);
        }

        let index: WorkspaceIndex = serde_json::from_slice(&fs::read(paths.workspace_path())?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let entry = index.tabs.into_iter().next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "workspace has no documents")
        })?;
        let content = fs::read_to_string(paths.documents_dir().join(format!(
            "{}.{}",
            entry.id,
            entry.kind.extension()
        )))?;

        Ok(Self {
            document: Document {
                id: entry.id,
                title: entry.title,
                kind: entry.kind,
                content,
                dirty: false,
            },
            paths,
        })
    }

    pub fn save_index(&self) -> io::Result<()> {
        let index = WorkspaceIndex {
            version: WORKSPACE_VERSION,
            tabs: vec![TabEntry {
                id: self.document.id,
                title: self.document.title.clone(),
                kind: self.document.kind,
            }],
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
