use directories::ProjectDirs;
use std::{fs, io, path::PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    data_dir: PathBuf,
}

impl AppPaths {
    pub fn new() -> io::Result<Self> {
        let project_dirs = ProjectDirs::from("", "", "Goatpad").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "could not determine Goatpad data directory",
            )
        })?;
        let paths = Self {
            data_dir: project_dirs.data_local_dir().to_path_buf(),
        };
        paths.ensure_exists()?;
        Ok(paths)
    }

    pub fn documents_dir(&self) -> PathBuf {
        self.data_dir.join("documents")
    }

    pub fn themes_dir(&self) -> PathBuf {
        self.data_dir.join("themes")
    }

    pub fn workspace_path(&self) -> PathBuf {
        self.data_dir.join("workspace.json")
    }

    pub fn session_path(&self) -> PathBuf {
        self.data_dir.join("session.json")
    }

    fn ensure_exists(&self) -> io::Result<()> {
        fs::create_dir_all(self.documents_dir())?;
        fs::create_dir_all(self.themes_dir())
    }
}
