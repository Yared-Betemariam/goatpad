use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocKind {
    Md,
    Txt,
}

impl DocKind {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Md => "md",
            Self::Txt => "txt",
        }
    }
}

impl Default for DocKind {
    fn default() -> Self {
        Self::Md
    }
}

#[derive(Debug)]
pub struct Document {
    pub id: Uuid,
    pub title: String,
    pub kind: DocKind,
    pub content: String,
    pub dirty: bool,
}

impl Document {
    pub fn new_untitled() -> Self {
        Self {
            id: Uuid::new_v4(),
            title: "Untitled".to_owned(),
            kind: DocKind::Md,
            content: String::new(),
            dirty: false,
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new_untitled()
    }
}
