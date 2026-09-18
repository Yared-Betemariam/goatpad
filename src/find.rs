use std::ops::Range;

/// Returns non-overlapping UTF-8 byte ranges for case-insensitive ASCII matches.
/// Non-ASCII text remains searchable with exact Unicode casing while every range
/// is guaranteed to fall on valid character boundaries.
pub fn find_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }

    let query_chars = query.chars().count();
    if query_chars == 0 {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut search_from = 0;
    while search_from < text.len() {
        let mut found = None;
        for (relative_start, _) in text[search_from..].char_indices() {
            let start = search_from + relative_start;
            let end = text[start..]
                .char_indices()
                .nth(query_chars)
                .map_or(text.len(), |(offset, _)| start + offset);
            let candidate = &text[start..end];
            if candidate.chars().count() == query_chars && candidate.eq_ignore_ascii_case(query) {
                found = Some(start..end);
                break;
            }
        }

        let Some(range) = found else {
            break;
        };
        search_from = range.end;
        ranges.push(range);
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::find_ranges;

    #[test]
    fn finds_non_overlapping_case_insensitive_matches() {
        assert_eq!(
            find_ranges("Note note NOTE", "note"),
            vec![0..4, 5..9, 10..14]
        );
        assert_eq!(find_ranges("aaaa", "aa"), vec![0..2, 2..4]);
    }

    #[test]
    fn returns_valid_ranges_for_unicode_text() {
        let text = "A 🐐 note and 🐐 note";
        let ranges = find_ranges(text, "🐐 note");
        assert_eq!(ranges.len(), 2);
        assert!(ranges.iter().all(|range| text.is_char_boundary(range.start)
            && text.is_char_boundary(range.end)
            && &text[range.clone()] == "🐐 note"));
    }

    #[test]
    fn an_empty_query_has_no_matches() {
        assert!(find_ranges("anything", "").is_empty());
    }
}
