use crate::document::DocKind;
use std::{
    fs, io,
    path::Path,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SaveRequest {
    pub id: Uuid,
    pub kind: DocKind,
    pub content: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct SaveResult {
    pub id: Uuid,
    pub result: Result<(), String>,
}

pub fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let temp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("tmp")
    ));
    fs::write(&temp_path, contents)?;
    fs::rename(temp_path, path)
}

pub fn start_writer_thread() -> (Sender<SaveRequest>, Receiver<SaveResult>) {
    let (sender, receiver) = mpsc::channel::<SaveRequest>();
    let (result_sender, result_receiver) = mpsc::channel::<SaveResult>();
    thread::Builder::new()
        .name("goatpad-writer".to_owned())
        .spawn(move || {
            while let Ok(request) = receiver.recv() {
                let result =
                    atomic_write(&request.path, request.content.as_bytes()).map_err(|error| {
                        format!(
                            "failed to save document {} ({}): {error}",
                            request.id,
                            request.kind.extension()
                        )
                    });
                let _ = result_sender.send(SaveResult {
                    id: request.id,
                    result,
                });
            }
        })
        .expect("failed to start Goatpad writer thread");
    (sender, result_receiver)
}

#[cfg(test)]
mod tests {
    use super::atomic_write;
    use std::fs;

    #[test]
    fn atomic_write_replaces_existing_contents() {
        let directory = std::env::temp_dir().join(format!("goatpad-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("note.md");
        fs::write(&path, "old").unwrap();

        atomic_write(&path, b"new").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert!(!path.with_extension("md.tmp").exists());
        fs::remove_dir_all(directory).unwrap();
    }
}
