use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DocKind {
    #[default]
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

pub const AUTO_TITLE_MAX_CHARS: usize = 20;
const UNTITLED_TITLE: &str = "Untitled";

#[derive(Debug)]
pub struct Document {
    pub id: Uuid,
    pub title: String,
    pub title_is_custom: bool,
    pub kind: DocKind,
    pub content: String,
    pub last_opened_at: u64,
    pub dirty: bool,
}

impl Document {
    pub fn new_untitled() -> Self {
        Self {
            id: Uuid::new_v4(),
            title: UNTITLED_TITLE.to_owned(),
            title_is_custom: false,
            kind: DocKind::Md,
            content: String::new(),
            last_opened_at: unix_timestamp_millis(),
            dirty: false,
        }
    }

    pub fn refresh_automatic_title(&mut self) -> bool {
        if self.title_is_custom {
            return false;
        }
        let title = automatic_title(&self.content);
        if self.title == title {
            return false;
        }
        self.title = title;
        true
    }

    pub fn rename(&mut self, title: &str) {
        let title = title.trim();
        if title.is_empty() {
            self.title_is_custom = false;
            self.title = automatic_title(&self.content);
        } else {
            self.title_is_custom = true;
            self.title = title.to_owned();
        }
    }
}

pub fn unix_timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub fn automatic_title(content: &str) -> String {
    let first_line = content.lines().next().unwrap_or_default().trim();
    if first_line.is_empty() {
        return UNTITLED_TITLE.to_owned();
    }

    let char_count = first_line.chars().count();
    let mut title: String = first_line.chars().take(AUTO_TITLE_MAX_CHARS).collect();
    let trimmed = title.trim_end();
    if trimmed.len() != title.len() {
        title = trimmed.to_owned();
    }
    if char_count > AUTO_TITLE_MAX_CHARS {
        title.push('…');
    }
    title
}

impl Default for Document {
    fn default() -> Self {
        Self::new_untitled()
    }
}

#[cfg(test)]
mod tests {
    use super::{AUTO_TITLE_MAX_CHARS, Document, automatic_title};

    #[test]
    fn automatic_title_uses_the_trimmed_first_line() {
        assert_eq!(automatic_title("  Shopping list  \nMilk"), "Shopping list");
        assert_eq!(automatic_title("\nSecond line"), "Untitled");
    }

    #[test]
    fn automatic_title_is_unicode_safe_and_truncated() {
        let content = "🦀".repeat(40);
        assert_eq!(
            automatic_title(&content),
            format!("{}…", "🦀".repeat(AUTO_TITLE_MAX_CHARS))
        );
    }

    #[test]
    fn automatic_title_trims_trailing_whitespace_on_truncation() {
        // "This is a long test line" has 24 chars. First 20 is "This is a long test "
        assert_eq!(
            automatic_title("This is a long test line"),
            "This is a long test…"
        );
    }

    #[test]
    fn a_custom_title_is_not_replaced_by_content_changes() {
        let mut document = Document::new_untitled();
        document.content = "Generated title".to_owned();
        assert!(document.refresh_automatic_title());
        document.rename("My note");
        document.content = "Changed first line".to_owned();

        assert!(!document.refresh_automatic_title());
        assert_eq!(document.title, "My note");
    }

    #[test]
    fn clearing_a_custom_title_restores_automatic_naming() {
        let mut document = Document::new_untitled();
        document.content = "Generated title".to_owned();
        document.rename("Custom");
        document.rename("   ");

        assert!(!document.title_is_custom);
        assert_eq!(document.title, "Generated title");
    }
}
