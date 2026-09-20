use crate::editor::formatting::byte_index;
use egui::{Key, Modifiers, text::CCursorRange};

/// The single contiguous change egui applied at the primary cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEditChange {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

/// Finds the one contiguous change between two editor contents.
///
/// egui processes a text edit event at a time, so a common-prefix/common-suffix
/// diff is enough to mirror normal typing, deletion, paste, and indentation
/// events at the secondary cursors.
pub fn detect_change(before: &str, after: &str) -> Option<TextEditChange> {
    if before == after {
        return None;
    }

    let before_chars: Vec<char> = before.chars().collect();
    let after_chars: Vec<char> = after.chars().collect();
    let prefix = before_chars
        .iter()
        .zip(&after_chars)
        .take_while(|(before, after)| before == after)
        .count();
    let remaining_before = before_chars.len() - prefix;
    let remaining_after = after_chars.len() - prefix;
    let suffix = before_chars[prefix..]
        .iter()
        .rev()
        .zip(after_chars[prefix..].iter().rev())
        .take_while(|(before, after)| before == after)
        .take(remaining_before.min(remaining_after))
        .count();

    Some(TextEditChange {
        start: prefix,
        end: before_chars.len() - suffix,
        replacement: after_chars[prefix..after_chars.len() - suffix]
            .iter()
            .collect(),
    })
}

/// Finds the primary edit using egui's cursor positions as a disambiguating
/// hint. This matters when the inserted text is identical to neighboring
/// characters, where a plain prefix/suffix diff cannot know the edit point.
pub fn detect_change_at(
    before: &str,
    after: &str,
    before_cursor: CCursorRange,
    primary_cursor_after: usize,
) -> Option<TextEditChange> {
    if before == after {
        return None;
    }

    let before_len = before.chars().count();
    let after_len = after.chars().count();
    let [selection_start, selection_end] = before_cursor.sorted_cursors();
    let selection_start = selection_start.index.0.min(before_len);
    let selection_end = selection_end.index.0.min(before_len);
    let primary_cursor_after = primary_cursor_after.min(after_len);

    if selection_start != selection_end {
        let removed = selection_end - selection_start;
        let inserted = after_len.saturating_sub(before_len - removed);
        let replacement_start = primary_cursor_after.saturating_sub(inserted);
        return Some(TextEditChange {
            start: selection_start,
            end: selection_end,
            replacement: chars_slice(after, replacement_start, replacement_start + inserted),
        });
    }

    let cursor = selection_start;
    if after_len > before_len {
        let inserted = after_len - before_len;
        let replacement_start = primary_cursor_after.saturating_sub(inserted);
        return Some(TextEditChange {
            start: cursor,
            end: cursor,
            replacement: chars_slice(after, replacement_start, primary_cursor_after),
        });
    }

    if after_len < before_len {
        let removed = before_len - after_len;
        let (start, end) = if primary_cursor_after < cursor {
            (cursor.saturating_sub(removed), cursor)
        } else {
            (cursor, cursor.saturating_add(removed).min(before_len))
        };
        return Some(TextEditChange {
            start,
            end,
            replacement: String::new(),
        });
    }

    detect_change(before, after)
}

#[derive(Clone, Copy)]
enum MirrorMode {
    Insert,
    BackwardDelete(usize),
    ForwardDelete(usize),
    ReplaceAtCaret,
}

#[derive(Clone)]
struct SecondaryEdit {
    source_offset: usize,
    start: usize,
    end: usize,
    replacement: String,
}

/// Mirrors the primary edit at the secondary cursors.
///
/// `text` must already contain the primary edit. The offsets in
/// `secondary_offsets` refer to the content before that primary edit. The
/// returned tuple contains the updated secondary offsets and the primary
/// cursor position after those edits have been applied.
pub fn apply_secondary_edits(
    text: &mut String,
    primary_change: &TextEditChange,
    before_cursor: CCursorRange,
    primary_cursor_after: usize,
    secondary_offsets: &[usize],
) -> (Vec<usize>, usize) {
    if secondary_offsets.is_empty() {
        return (Vec::new(), primary_cursor_after);
    }

    let mode = mirror_mode(primary_change, before_cursor);
    let mut pending = Vec::new();
    let mut seen_mapped_offsets = Vec::new();
    let text_len = text.chars().count();

    for &source_offset in secondary_offsets {
        let mapped_offset = map_offset_through_change(source_offset, primary_change);
        if mapped_offset == primary_cursor_after
            || !seen_mapped_offsets
                .iter()
                .all(|offset| *offset != mapped_offset)
        {
            continue;
        }
        seen_mapped_offsets.push(mapped_offset);

        let (start, end) = match mode {
            MirrorMode::Insert | MirrorMode::ReplaceAtCaret => {
                (mapped_offset.min(text_len), mapped_offset.min(text_len))
            }
            MirrorMode::BackwardDelete(count) => {
                let end = mapped_offset.min(text_len);
                (end.saturating_sub(count), end)
            }
            MirrorMode::ForwardDelete(count) => {
                let start = mapped_offset.min(text_len);
                (start, start.saturating_add(count).min(text_len))
            }
        };

        pending.push(SecondaryEdit {
            source_offset,
            start,
            end,
            replacement: primary_change.replacement.clone(),
        });
    }

    // Apply from right to left so every operation remains expressed in the
    // coordinates of the content after the primary edit.
    pending.sort_by(|left, right| {
        right
            .start
            .cmp(&left.start)
            .then_with(|| right.end.cmp(&left.end))
    });
    for edit in &pending {
        text.replace_range(
            byte_index(text, edit.start)..byte_index(text, edit.end),
            &edit.replacement,
        );
    }

    let mut updated_primary = primary_cursor_after;
    for edit in pending.iter().rev() {
        updated_primary = map_position_through_edit(updated_primary, edit, false);
    }

    let mut updated_secondary = Vec::with_capacity(pending.len());
    for edit in pending.iter().rev() {
        let mut offset = map_offset_through_change(edit.source_offset, primary_change);
        for mapped_edit in pending.iter().rev() {
            let is_own_edit = mapped_edit.source_offset == edit.source_offset;
            offset = map_position_through_edit(offset, mapped_edit, is_own_edit);
        }
        if offset != updated_primary && updated_secondary.iter().all(|existing| *existing != offset)
        {
            updated_secondary.push(offset);
        }
    }
    updated_secondary.sort_unstable();

    (updated_secondary, updated_primary)
}

/// Moves secondary carets for the basic navigation keys supported by the
/// editor. More complex selection moves deliberately return `None`, allowing
/// the caller to fall back to a single caret rather than presenting stale
/// secondary positions.
pub fn move_secondary_offsets(
    text: &str,
    before_cursor: CCursorRange,
    primary_cursor_after: usize,
    secondary_offsets: &[usize],
    key: Key,
    modifiers: Modifiers,
) -> Option<Vec<usize>> {
    if secondary_offsets.is_empty() || !before_cursor.is_empty() || modifiers.shift {
        return None;
    }

    let chars: Vec<char> = text.chars().collect();
    let before = before_cursor.primary.index.0.min(chars.len());
    let after = primary_cursor_after.min(chars.len());
    let mut moved: Vec<usize> = match key {
        Key::ArrowLeft | Key::ArrowRight
            if !modifiers.ctrl && !modifiers.alt && !modifiers.mac_cmd =>
        {
            let delta = after as isize - before as isize;
            secondary_offsets
                .iter()
                .map(|offset| {
                    if delta.is_negative() {
                        offset.saturating_sub(delta.unsigned_abs())
                    } else {
                        offset.saturating_add(delta as usize).min(chars.len())
                    }
                })
                .collect()
        }
        Key::Home | Key::End => {
            if modifiers.ctrl || modifiers.command {
                let target = if key == Key::Home { 0 } else { chars.len() };
                secondary_offsets.iter().map(|_| target).collect()
            } else {
                secondary_offsets
                    .iter()
                    .map(|offset| line_edge(&chars, *offset, key == Key::End))
                    .collect()
            }
        }
        Key::ArrowUp | Key::ArrowDown if modifiers.ctrl || modifiers.command => {
            let target = if key == Key::ArrowUp { 0 } else { chars.len() };
            secondary_offsets.iter().map(|_| target).collect()
        }
        Key::ArrowUp | Key::ArrowDown => {
            let before_line = line_and_column(&chars, before).0;
            let after_line = line_and_column(&chars, after).0;
            let line_delta = after_line as isize - before_line as isize;
            secondary_offsets
                .iter()
                .map(|offset| {
                    let (line, column) = line_and_column(&chars, *offset);
                    let target_line = if line_delta.is_negative() {
                        line.saturating_sub(line_delta.unsigned_abs())
                    } else {
                        line.saturating_add(line_delta as usize)
                    };
                    offset_for_line_column(&chars, target_line, column)
                })
                .collect()
        }
        _ => return None,
    };

    moved.sort_unstable();
    moved.dedup();
    Some(moved)
}

fn line_edge(chars: &[char], offset: usize, end: bool) -> usize {
    let offset = offset.min(chars.len());
    if end {
        chars[offset..]
            .iter()
            .position(|character| *character == '\n')
            .map_or(chars.len(), |relative| offset + relative)
    } else {
        chars[..offset]
            .iter()
            .rposition(|character| *character == '\n')
            .map_or(0, |newline| newline + 1)
    }
}

fn line_and_column(chars: &[char], offset: usize) -> (usize, usize) {
    let mut line = 0;
    let mut line_start = 0;
    for (index, character) in chars.iter().enumerate().take(offset.min(chars.len())) {
        if *character == '\n' {
            line += 1;
            line_start = index + 1;
        }
    }
    (line, offset.min(chars.len()).saturating_sub(line_start))
}

fn offset_for_line_column(chars: &[char], target_line: usize, column: usize) -> usize {
    let mut line = 0;
    let mut line_start = 0;
    for (index, character) in chars.iter().enumerate() {
        if line == target_line && (*character == '\n' || index == line_start + column) {
            return index.min(line_start + column);
        }
        if *character == '\n' {
            if line == target_line {
                return index;
            }
            line += 1;
            line_start = index + 1;
        }
    }
    if line == target_line {
        line_start + column.min(chars.len().saturating_sub(line_start))
    } else {
        chars.len()
    }
}

fn mirror_mode(change: &TextEditChange, before_cursor: CCursorRange) -> MirrorMode {
    if !before_cursor.is_empty() {
        return MirrorMode::ReplaceAtCaret;
    }

    let cursor = before_cursor.primary.index.0;
    let removed = change.end.saturating_sub(change.start);
    if removed == 0 {
        MirrorMode::Insert
    } else if change.end == cursor {
        MirrorMode::BackwardDelete(removed)
    } else if change.start == cursor {
        MirrorMode::ForwardDelete(removed)
    } else {
        MirrorMode::ReplaceAtCaret
    }
}

fn chars_slice(text: &str, start: usize, end: usize) -> String {
    text.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

fn map_offset_through_change(offset: usize, change: &TextEditChange) -> usize {
    let inserted = change.replacement.chars().count();
    let removed = change.end.saturating_sub(change.start);
    if offset < change.start {
        offset
    } else if offset >= change.end {
        offset + inserted - removed
    } else {
        change.start + inserted
    }
}

fn map_position_through_edit(position: usize, edit: &SecondaryEdit, own_edit: bool) -> usize {
    let inserted = edit.replacement.chars().count();
    let removed = edit.end.saturating_sub(edit.start);
    if removed == 0 {
        if position > edit.start || (own_edit && position == edit.start) {
            position + inserted
        } else {
            position
        }
    } else if position < edit.start {
        position
    } else if position >= edit.end {
        position + inserted - removed
    } else {
        edit.start + inserted
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_secondary_edits, detect_change, detect_change_at};
    use egui::text::{CCursor, CCursorRange};

    #[test]
    fn detects_unicode_safe_changes() {
        let change = detect_change("café", "ca🦀é").unwrap();
        assert_eq!(change.start, 2);
        assert_eq!(change.end, 3);
        assert_eq!(change.replacement, "🦀");
    }

    #[test]
    fn uses_the_primary_cursor_when_inserted_text_repeats_neighbors() {
        let change =
            detect_change_at("aaa", "aaaa", CCursorRange::one(CCursor::new(1)), 2).unwrap();
        assert_eq!(change.start, 1);
        assert_eq!(change.end, 1);
        assert_eq!(change.replacement, "a");
    }

    #[test]
    fn mirrors_insertions_and_updates_all_offsets() {
        let before = "one\ntwo";
        let mut after = "one!\ntwo".to_owned();
        let change = detect_change(before, &after).unwrap();

        let (offsets, primary) = apply_secondary_edits(
            &mut after,
            &change,
            CCursorRange::one(CCursor::new(3)),
            4,
            &[0, 4],
        );

        assert_eq!(after, "!one!\n!two");
        assert_eq!(offsets, vec![1, 7]);
        assert_eq!(primary, 5);
    }

    #[test]
    fn mirrors_backward_deletions() {
        let before = "abc\ndef";
        let mut after = "ab\ndef".to_owned();
        let change = detect_change(before, &after).unwrap();

        let (offsets, primary) = apply_secondary_edits(
            &mut after,
            &change,
            CCursorRange::one(CCursor::new(3)),
            2,
            &[1, 5],
        );

        assert_eq!(after, "b\nef");
        assert_eq!(offsets, vec![0, 3]);
        assert_eq!(primary, 1);
    }
}
