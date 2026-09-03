use egui::text::{CCursor, CCursorRange};

/// Converts a character offset into the matching byte offset for `text`.
pub fn byte_index(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(index, _)| index)
}

/// Reports the 1-based line and column of `cursor_offset` (in characters) within `content`.
pub fn cursor_position(content: &str, cursor_offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for character in content.chars().take(cursor_offset) {
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

/// Reports the line-ending convention already present in `content`.
pub const fn line_ending_label(content: &str) -> &'static str {
    if contains(content, "\r\n") {
        "Windows (CRLF)"
    } else if contains(content, "\r") {
        "Classic Mac (CR)"
    } else {
        "Unix (LF)"
    }
}

/// `str::contains` is not `const`, so this is a small hand-rolled substitute.
const fn contains(haystack: &str, needle: &str) -> bool {
    let haystack = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.len() > haystack.len() {
        return false;
    }
    let mut start = 0;
    while start + needle.len() <= haystack.len() {
        let mut matches = true;
        let mut offset = 0;
        while offset < needle.len() {
            if haystack[start + offset] != needle[offset] {
                matches = false;
                break;
            }
            offset += 1;
        }
        if matches {
            return true;
        }
        start += 1;
    }
    false
}

fn ordered(start: usize, end: usize) -> (usize, usize) {
    if start <= end {
        (start, end)
    } else {
        (end, start)
    }
}

/// Wraps the selected characters in `open`/`close`, or inserts an empty pair at the cursor.
pub fn wrap_selection(
    text: &mut String,
    start: usize,
    end: usize,
    open: &str,
    close: &str,
) -> CCursorRange {
    let (start, end) = ordered(start, end);
    let start = start.min(text.chars().count());
    let end = end.min(text.chars().count());
    let start_byte = byte_index(text, start);
    let end_byte = byte_index(text, end);
    text.insert_str(end_byte, close);
    text.insert_str(start_byte, open);
    if start == end {
        CCursorRange::one(CCursor::new(start + open.chars().count()))
    } else {
        CCursorRange::two(
            CCursor::new(start + open.chars().count()),
            CCursor::new(end + open.chars().count()),
        )
    }
}

/// Expands the given character range to cover every full line it touches.
fn line_bounds(text: &str, start: usize, end: usize) -> (usize, usize) {
    let start_byte = byte_index(text, start.min(text.chars().count()));
    let end_byte = byte_index(text, end.min(text.chars().count()));
    let line_start = text[..start_byte].rfind('\n').map_or(0, |index| index + 1);
    let line_end = text[end_byte..]
        .find('\n')
        .map_or(text.len(), |index| end_byte + index);
    (line_start, line_end)
}

fn replace_lines(
    text: &mut String,
    line_start: usize,
    line_end: usize,
    replacement: &str,
) -> CCursorRange {
    let replacement_chars = replacement.chars().count();
    text.replace_range(line_start..line_end, replacement);
    let prefix_chars = text[..line_start].chars().count();
    CCursorRange::two(
        CCursor::new(prefix_chars),
        CCursor::new(prefix_chars + replacement_chars),
    )
}

/// Toggles a `- ` bullet-list marker on every selected line.
pub fn toggle_bullet_list(text: &mut String, start: usize, end: usize) -> CCursorRange {
    let (line_start, line_end) = line_bounds(text, start, end);
    let lines: Vec<&str> = text[line_start..line_end].split('\n').collect();
    let remove = lines.iter().all(|line| line.starts_with("- "));
    let replacement = lines
        .into_iter()
        .map(|line| {
            if remove {
                line.strip_prefix("- ").unwrap_or(line).to_owned()
            } else {
                format!("- {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    replace_lines(text, line_start, line_end, &replacement)
}

fn strip_numbered_prefix(line: &str) -> Option<&str> {
    let digits_end = line.find(|character: char| !character.is_ascii_digit())?;
    if digits_end == 0 {
        return None;
    }
    line[digits_end..].strip_prefix(". ")
}

/// Toggles sequential `1. `-style numbering on every selected line.
pub fn toggle_numbered_list(text: &mut String, start: usize, end: usize) -> CCursorRange {
    let (line_start, line_end) = line_bounds(text, start, end);
    let lines: Vec<&str> = text[line_start..line_end].split('\n').collect();
    let remove = lines
        .iter()
        .all(|line| strip_numbered_prefix(line).is_some());
    let replacement = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if remove {
                strip_numbered_prefix(line).unwrap_or(line).to_owned()
            } else {
                format!("{}. {line}", index + 1)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    replace_lines(text, line_start, line_end, &replacement)
}

fn heading_level(line: &str) -> Option<u8> {
    let hashes = line
        .chars()
        .take_while(|&character| character == '#')
        .count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &line[hashes..];
    (rest.is_empty() || rest.starts_with(' ')).then_some(hashes as u8)
}

fn strip_heading(line: &str) -> &str {
    match heading_level(line) {
        Some(level) => line[level as usize..].trim_start_matches(' '),
        None => line,
    }
}

/// Toggles a `#`-style heading of the given `level` (1-6) on every selected line.
pub fn set_heading(text: &mut String, start: usize, end: usize, level: u8) -> CCursorRange {
    let (line_start, line_end) = line_bounds(text, start, end);
    let lines: Vec<&str> = text[line_start..line_end].split('\n').collect();
    let remove = lines.iter().all(|line| heading_level(line) == Some(level));
    let replacement = lines
        .into_iter()
        .map(|line| {
            let bare = strip_heading(line);
            if remove {
                bare.to_owned()
            } else {
                format!("{} {bare}", "#".repeat(level as usize))
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    replace_lines(text, line_start, line_end, &replacement)
}

/// Inserts a Markdown link at the cursor, wrapping the selection as the link label.
pub fn insert_link(text: &mut String, start: usize, end: usize) -> CCursorRange {
    let (start, end) = ordered(start, end);
    let start = start.min(text.chars().count());
    let end = end.min(text.chars().count());
    let start_byte = byte_index(text, start);
    let end_byte = byte_index(text, end);
    let label = if start == end {
        "link text".to_owned()
    } else {
        text[start_byte..end_byte].to_owned()
    };
    let label_chars = label.chars().count();
    let replacement = format!("[{label}](url)");
    text.replace_range(start_byte..end_byte, &replacement);
    let url_start = start + label_chars + "[](".chars().count();
    let url_end = url_start + "url".chars().count();
    CCursorRange::two(CCursor::new(url_start), CCursor::new(url_end))
}

/// Inserts a starter Markdown table at the cursor.
pub fn insert_table(text: &mut String, start: usize, end: usize) -> CCursorRange {
    let (start, end) = ordered(start, end);
    let start = start.min(text.chars().count());
    let end = end.min(text.chars().count());
    let start_byte = byte_index(text, start);
    let end_byte = byte_index(text, end);

    let needs_leading_newline = start_byte > 0 && !text[..start_byte].ends_with('\n');
    let needs_trailing_newline = end_byte < text.len() && !text[end_byte..].starts_with('\n');

    let mut template = String::new();
    if needs_leading_newline {
        template.push('\n');
    }
    template.push_str("| Column 1 | Column 2 |\n| --- | --- |\n| Cell | Cell |");
    if needs_trailing_newline {
        template.push('\n');
    }

    let template_chars = template.chars().count();
    text.replace_range(start_byte..end_byte, &template);
    CCursorRange::one(CCursor::new(start + template_chars))
}

fn find_from(characters: &[char], from: usize, target: char) -> Option<usize> {
    characters[from..]
        .iter()
        .position(|&character| character == target)
        .map(|offset| from + offset)
}

fn strip_links(input: &str) -> String {
    let characters: Vec<char> = input.chars().collect();
    let mut result = String::with_capacity(input.len());
    let mut index = 0;
    while index < characters.len() {
        let link = (characters[index] == '[')
            .then(|| find_from(&characters, index + 1, ']'))
            .flatten()
            .filter(|&close_bracket| characters.get(close_bracket + 1) == Some(&'('))
            .and_then(|close_bracket| {
                find_from(&characters, close_bracket + 2, ')')
                    .map(|close_paren| (close_bracket, close_paren))
            });
        if let Some((close_bracket, close_paren)) = link {
            result.extend(&characters[index + 1..close_bracket]);
            index = close_paren + 1;
            continue;
        }
        result.push(characters[index]);
        index += 1;
    }
    result
}

fn strip_inline_markup(line: &str) -> String {
    let mut text = strip_links(line);
    for marker in ["***", "___", "**", "__", "~~", "*", "_", "`"] {
        text = text.replace(marker, "");
    }
    text
}

fn strip_list_marker(line: &str) -> &str {
    if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
        return rest;
    }
    strip_numbered_prefix(line).unwrap_or(line)
}

/// Strips common Markdown syntax (emphasis, headings, lists, links) from the selected lines.
pub fn clear_formatting(text: &mut String, start: usize, end: usize) -> CCursorRange {
    let (line_start, line_end) = line_bounds(text, start, end);
    let replacement = text[line_start..line_end]
        .lines()
        .map(|line| strip_inline_markup(strip_list_marker(strip_heading(line))))
        .collect::<Vec<_>>()
        .join("\n");
    replace_lines(text, line_start, line_end, &replacement)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_starts_at_line_one_column_one() {
        assert_eq!(cursor_position("", 0), (1, 1));
    }

    #[test]
    fn cursor_position_tracks_newlines() {
        assert_eq!(cursor_position("first\nsecond", 6), (2, 1));
    }

    #[test]
    fn cursor_position_counts_unicode_characters() {
        assert_eq!(cursor_position("café\n🦀", 6), (2, 2));
    }

    #[test]
    fn line_ending_is_detected_from_content() {
        assert_eq!(line_ending_label("one\r\ntwo"), "Windows (CRLF)");
        assert_eq!(line_ending_label("one\ntwo"), "Unix (LF)");
        assert_eq!(line_ending_label("one\rtwo"), "Classic Mac (CR)");
    }

    #[test]
    fn formatting_wraps_a_selection_and_leaves_it_selected() {
        let mut text = "hello world".to_owned();
        let range = wrap_selection(&mut text, 6, 11, "**", "**");
        assert_eq!(text, "hello **world**");
        assert_eq!(
            (
                range.primary.index.0.min(range.secondary.index.0),
                range.primary.index.0.max(range.secondary.index.0)
            ),
            (8, 13)
        );
    }

    #[test]
    fn bullet_list_toggle_adds_then_removes_each_selected_line() {
        let mut text = "one\ntwo".to_owned();
        let length = text.chars().count();
        toggle_bullet_list(&mut text, 0, length);
        assert_eq!(text, "- one\n- two");
        let length = text.chars().count();
        toggle_bullet_list(&mut text, 0, length);
        assert_eq!(text, "one\ntwo");
    }

    #[test]
    fn numbered_list_toggle_adds_then_removes_each_selected_line() {
        let mut text = "one\ntwo".to_owned();
        let length = text.chars().count();
        toggle_numbered_list(&mut text, 0, length);
        assert_eq!(text, "1. one\n2. two");
        let length = text.chars().count();
        toggle_numbered_list(&mut text, 0, length);
        assert_eq!(text, "one\ntwo");
    }

    #[test]
    fn heading_toggle_adds_then_removes_the_marker() {
        let mut text = "Title".to_owned();
        set_heading(&mut text, 0, 5, 2);
        assert_eq!(text, "## Title");
        set_heading(&mut text, 0, 8, 2);
        assert_eq!(text, "Title");
    }

    #[test]
    fn heading_toggle_replaces_a_different_level() {
        let mut text = "# Title".to_owned();
        set_heading(&mut text, 0, 7, 3);
        assert_eq!(text, "### Title");
    }

    #[test]
    fn insert_link_wraps_the_selection_and_selects_the_url() {
        let mut text = "see docs".to_owned();
        let range = insert_link(&mut text, 4, 8);
        assert_eq!(text, "see [docs](url)");
        assert_eq!(
            (
                range.primary.index.0.min(range.secondary.index.0),
                range.primary.index.0.max(range.secondary.index.0)
            ),
            (11, 14)
        );
    }

    #[test]
    fn insert_table_adds_a_starter_template_on_its_own_lines() {
        let mut text = "notes".to_owned();
        insert_table(&mut text, 5, 5);
        assert_eq!(
            text,
            "notes\n| Column 1 | Column 2 |\n| --- | --- |\n| Cell | Cell |"
        );
    }

    #[test]
    fn clear_formatting_strips_common_markdown_syntax() {
        let mut text = "# Heading\n- **bold** and [a link](https://example.com)".to_owned();
        let length = text.chars().count();
        clear_formatting(&mut text, 0, length);
        assert_eq!(text, "Heading\nbold and a link");
    }
}
