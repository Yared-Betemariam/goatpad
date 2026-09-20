use egui::{
    Color32, FontFamily, FontId, Stroke,
    text::{LayoutJob, TextFormat},
};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MarkdownStyle {
    Heading,
    Strong,
    Emphasis,
    Code,
    Link,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MarkdownPalette {
    primary: Color32,
    secondary: Color32,
}

impl MarkdownPalette {
    pub(crate) const fn new(primary: Color32, secondary: Color32) -> Self {
        Self { primary, secondary }
    }

    pub(crate) fn format(
        self,
        style: MarkdownStyle,
        zoom: f32,
        font_family: &FontFamily,
        text_color: Color32,
    ) -> TextFormat {
        style.format(zoom, font_family, text_color, self)
    }
}

impl MarkdownStyle {
    fn priority(self) -> u8 {
        match self {
            Self::Heading => 6,
            Self::Code => 5,
            Self::Link => 4,
            Self::Strong => 3,
            Self::Emphasis => 2,
            Self::List => 1,
        }
    }

    fn format(
        self,
        zoom: f32,
        font_family: &FontFamily,
        text_color: Color32,
        palette: MarkdownPalette,
    ) -> TextFormat {
        let mut format = default_format(zoom, font_family, text_color);
        match self {
            Self::Heading | Self::Link => {
                format.color = palette.primary;
                if self == Self::Link {
                    format.underline = Stroke::new(1.0, palette.primary);
                }
            }
            Self::Strong => {}
            Self::Emphasis => format.italics = true,
            Self::Code => {
                format.font_id = FontId::new(16.0 * zoom, FontFamily::Monospace);
                format.color = palette.secondary;
                format.background = palette.secondary.gamma_multiply(0.20);
            }
            Self::List => {
                format.color = palette.secondary;
            }
        }
        format
    }
}

fn default_format(zoom: f32, font_family: &FontFamily, text_color: Color32) -> TextFormat {
    TextFormat::simple(FontId::new(16.0 * zoom, font_family.clone()), text_color)
}

/// The color used to underline misspelled words, matching Notepad's spell-check styling.
pub const MISSPELLED_UNDERLINE_COLOR: Color32 = Color32::from_rgb(224, 49, 49);
pub const FIND_HIGHLIGHT_COLOR: Color32 = Color32::from_rgb(255, 221, 64);

/// Marks `format` as misspelled by drawing a red underline, preserving every
/// other attribute (color, weight, background, …) it already carries.
fn mark_misspelled(mut format: TextFormat, zoom: f32) -> TextFormat {
    format.underline = Stroke::new((1.3 * zoom).max(1.0), MISSPELLED_UNDERLINE_COLOR);
    format
}

fn mark_find_match(mut format: TextFormat) -> TextFormat {
    format.background = FIND_HIGHLIGHT_COLOR;
    format.color = Color32::BLACK;
    format
}

/// Produces a live Markdown layout while preserving the editor's original text.
/// `zoom` scales every font size uniformly, mirroring Notepad's zoom control.
/// `palette` supplies the active theme's primary and secondary syntax colors.
/// `misspelled` lists the UTF-8 byte ranges that should be underlined in red.
/// `find_matches` lists ranges that should receive a yellow background.
pub(crate) fn highlight(
    text: &str,
    zoom: f32,
    font_family: &FontFamily,
    text_color: Color32,
    palette: MarkdownPalette,
    misspelled: &[Range<usize>],
    find_matches: &[Range<usize>],
) -> LayoutJob {
    let mut spans = Vec::<(Range<usize>, MarkdownStyle)>::new();
    let mut active = Vec::<MarkdownStyle>::new();

    for (event, range) in Parser::new_ext(text, Options::all()).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if let Some(style) = style_for_tag(&tag) {
                    active.push(style);
                    push_style_span(text, &mut spans, range, style);
                }
            }
            Event::End(tag_end) => {
                if let Some(style) = style_for_end(tag_end) {
                    push_style_span(text, &mut spans, range, style);
                    if let Some(index) = active.iter().rposition(|current| *current == style) {
                        active.remove(index);
                    }
                }
            }
            Event::Code(_) => push_style_span(text, &mut spans, range, MarkdownStyle::Code),
            Event::Text(_) | Event::Html(_) | Event::InlineHtml(_) => {
                if let Some(style) = active.iter().max_by_key(|style| style.priority()) {
                    push_style_span(text, &mut spans, range, *style);
                }
            }
            _ => {}
        }
    }

    layout_with_spans(
        text,
        spans,
        zoom,
        font_family,
        text_color,
        palette,
        misspelled,
        find_matches,
    )
}

/// Produces a plain-text layout, underlining misspelled words (given as
/// UTF-8 byte ranges) in red.
pub(crate) fn plain(
    text: &str,
    zoom: f32,
    font_family: &FontFamily,
    text_color: Color32,
    misspelled: &[Range<usize>],
    find_matches: &[Range<usize>],
) -> LayoutJob {
    layout_with_spans(
        text,
        Vec::new(),
        zoom,
        font_family,
        text_color,
        MarkdownPalette::new(text_color, text_color),
        misspelled,
        find_matches,
    )
}

fn style_for_tag(tag: &Tag<'_>) -> Option<MarkdownStyle> {
    match tag {
        Tag::Heading { .. } => Some(MarkdownStyle::Heading),
        Tag::Strong => Some(MarkdownStyle::Strong),
        Tag::Emphasis => Some(MarkdownStyle::Emphasis),
        Tag::Link { .. } => Some(MarkdownStyle::Link),
        Tag::CodeBlock(_) => Some(MarkdownStyle::Code),
        Tag::Item => Some(MarkdownStyle::List),
        _ => None,
    }
}

fn style_for_end(tag: TagEnd) -> Option<MarkdownStyle> {
    match tag {
        TagEnd::Heading(_) => Some(MarkdownStyle::Heading),
        TagEnd::Strong => Some(MarkdownStyle::Strong),
        TagEnd::Emphasis => Some(MarkdownStyle::Emphasis),
        TagEnd::Link => Some(MarkdownStyle::Link),
        TagEnd::CodeBlock => Some(MarkdownStyle::Code),
        TagEnd::Item => Some(MarkdownStyle::List),
        _ => None,
    }
}

fn push_style_span(
    text: &str,
    spans: &mut Vec<(Range<usize>, MarkdownStyle)>,
    range: Range<usize>,
    style: MarkdownStyle,
) {
    if style == MarkdownStyle::List {
        spans.extend(
            list_line_ranges(text, range)
                .into_iter()
                .map(|range| (range, style)),
        );
    } else {
        spans.push((range, style));
    }
}

/// Keeps list styling on list lines instead of allowing a list item's parser
/// range to bleed into an unmarked lazy continuation line.
pub(crate) fn list_line_ranges(text: &str, range: Range<usize>) -> Vec<Range<usize>> {
    let start = range.start.min(text.len());
    let end = range.end.min(text.len());
    if start >= end {
        return Vec::new();
    }

    let mut line_start = text[..start].rfind('\n').map_or(0, |newline| newline + 1);
    let mut result = Vec::new();

    while line_start < end {
        let line_end = text[line_start..]
            .find('\n')
            .map_or(text.len(), |newline| line_start + newline + 1);
        if is_list_line(&text[line_start..line_end]) {
            let clipped_start = start.max(line_start);
            let clipped_end = end.min(line_end);
            if clipped_start < clipped_end {
                result.push(clipped_start..clipped_end);
            }
        }
        if line_end >= end {
            break;
        }
        line_start = line_end;
    }

    result
}

fn is_list_line(line: &str) -> bool {
    let line = line.trim_end_matches(['\n', '\r']);
    let bytes = line.as_bytes();
    let mut marker_start = 0;
    while marker_start < bytes.len() && bytes[marker_start] == b' ' && marker_start < 3 {
        marker_start += 1;
    }

    let Some(&marker) = bytes.get(marker_start) else {
        return false;
    };
    if matches!(marker, b'-' | b'+' | b'*') {
        return bytes
            .get(marker_start + 1)
            .is_none_or(|byte| byte.is_ascii_whitespace());
    }

    let digits_start = marker_start;
    let mut digits_end = digits_start;
    while digits_end < bytes.len()
        && bytes[digits_end].is_ascii_digit()
        && digits_end - digits_start < 9
    {
        digits_end += 1;
    }
    digits_end > digits_start
        && bytes
            .get(digits_end)
            .is_some_and(|byte| matches!(byte, b'.' | b')'))
        && bytes
            .get(digits_end + 1)
            .is_none_or(|byte| byte.is_ascii_whitespace())
}

fn layout_with_spans(
    text: &str,
    spans: Vec<(Range<usize>, MarkdownStyle)>,
    zoom: f32,
    font_family: &FontFamily,
    text_color: Color32,
    palette: MarkdownPalette,
    misspelled: &[Range<usize>],
    find_matches: &[Range<usize>],
) -> LayoutJob {
    let mut styles = vec![None; text.len()];
    for (range, style) in spans {
        for current_style in styles
            .iter_mut()
            .take(range.end.min(text.len()))
            .skip(range.start.min(text.len()))
        {
            if current_style
                .is_none_or(|current: MarkdownStyle| style.priority() >= current.priority())
            {
                *current_style = Some(style);
            }
        }
    }
    let misspelled_flags = range_flags(text.len(), misspelled);
    let find_flags = range_flags(text.len(), find_matches);

    let format_at = |style: Option<MarkdownStyle>, is_misspelled: bool, is_find_match: bool| {
        let format = style.map_or_else(
            || default_format(zoom, font_family, text_color),
            |style| palette.format(style, zoom, font_family, text_color),
        );
        let format = if is_misspelled {
            mark_misspelled(format, zoom)
        } else {
            format
        };
        if is_find_match {
            mark_find_match(format)
        } else {
            format
        }
    };
    let mut job = LayoutJob::default();
    let mut start = 0;
    let mut current = (
        styles.first().copied().flatten(),
        misspelled_flags.first().copied().unwrap_or(false),
        find_flags.first().copied().unwrap_or(false),
    );
    for (index, _) in text.char_indices().skip(1) {
        let key = (styles[index], misspelled_flags[index], find_flags[index]);
        if key != current {
            job.append(
                &text[start..index],
                0.0,
                format_at(current.0, current.1, current.2),
            );
            start = index;
            current = key;
        }
    }
    if !text.is_empty() {
        job.append(
            &text[start..],
            0.0,
            format_at(current.0, current.1, current.2),
        );
    }
    job
}

/// Builds a per-byte boolean mask marking which positions in a `len`-byte
/// text fall inside one of the given misspelled-word ranges.
fn range_flags(len: usize, ranges: &[Range<usize>]) -> Vec<bool> {
    let mut flags = vec![false; len];
    for range in ranges {
        for flag in flags
            .iter_mut()
            .take(range.end.min(len))
            .skip(range.start.min(len))
        {
            *flag = true;
        }
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::{MarkdownPalette, highlight, plain};
    use egui::{Color32, FontFamily};

    fn palette() -> MarkdownPalette {
        MarkdownPalette::new(
            Color32::from_rgb(42, 91, 166),
            Color32::from_rgb(35, 120, 70),
        )
    }

    #[test]
    fn highlighting_covers_the_entire_document() {
        let job = highlight(
            "# Heading\n\nA **bold** [link](https://example.com).",
            1.0,
            &FontFamily::Proportional,
            Color32::DARK_GRAY,
            palette(),
            &[],
            &[],
        );
        assert_eq!(
            job.text,
            "# Heading\n\nA **bold** [link](https://example.com)."
        );
        assert_eq!(job.sections.first().unwrap().byte_range.start.0, 0);
        assert_eq!(
            job.sections.last().unwrap().byte_range.end.0,
            job.text.len()
        );
    }

    #[test]
    fn plain_text_uses_the_supplied_theme_colour() {
        let job = plain(
            "Theme-aware text",
            1.0,
            &FontFamily::Proportional,
            Color32::DARK_GRAY,
            &[],
            &[],
        );

        assert_eq!(job.sections[0].format.color, Color32::DARK_GRAY);
    }

    #[test]
    fn plain_text_underlines_misspelled_ranges_in_red() {
        let text = "a mispelled word";
        let word_range = 2..12; // "mispelled"
        let job = plain(
            text,
            1.0,
            &FontFamily::Proportional,
            Color32::DARK_GRAY,
            &[word_range.clone()],
            &[],
        );

        for section in &job.sections {
            let start = section.byte_range.start.0;
            let end = section.byte_range.end.0;
            let expected_underline = start >= word_range.start && end <= word_range.end;
            assert_eq!(
                section.format.underline.width > 0.0,
                expected_underline,
                "section {start}..{end} underline mismatch"
            );
        }
    }

    #[test]
    fn highlight_underlines_misspelled_words_without_losing_markdown_style() {
        let text = "**mispelled**";
        let job = highlight(
            text,
            1.0,
            &FontFamily::Proportional,
            Color32::DARK_GRAY,
            palette(),
            &[2..12],
            &[],
        );

        let misspelled_section = job
            .sections
            .iter()
            .find(|section| section.format.underline.width > 0.0)
            .expect("expected a misspelled section to be underlined");
        // The word keeps the normal editor font size and is also underlined in red.
        assert!(misspelled_section.format.font_id.size == 16.0);
        assert_eq!(
            misspelled_section.format.underline.color,
            super::MISSPELLED_UNDERLINE_COLOR
        );
    }

    #[test]
    fn find_matches_receive_a_yellow_background() {
        let job = plain(
            "find this text",
            1.0,
            &FontFamily::Proportional,
            Color32::DARK_GRAY,
            &[],
            &[5..9],
        );
        let matched = job
            .sections
            .iter()
            .find(|section| section.byte_range.start.0 == 5)
            .expect("find match should have its own section");
        assert_eq!(matched.format.background, super::FIND_HIGHLIGHT_COLOR);
        assert_eq!(matched.format.color, Color32::BLACK);
    }

    #[test]
    fn markdown_constructs_receive_distinct_formats() {
        let job = highlight(
            "# Heading\n*italic* **bold** `code` [link](https://example.com)\n- item",
            1.0,
            &FontFamily::Proportional,
            Color32::DARK_GRAY,
            palette(),
            &[],
            &[],
        );
        let formats = job
            .sections
            .iter()
            .map(|section| &section.format)
            .collect::<Vec<_>>();

        assert!(formats.iter().all(|format| format.font_id.size == 16.0));
        assert!(formats.iter().any(|format| format.italics));
        assert!(formats.iter().any(|format| format.font_id.size == 16.0));
        assert!(
            formats
                .iter()
                .any(|format| format.font_id.family == egui::FontFamily::Monospace)
        );
        assert!(formats.iter().any(|format| format.underline.width > 0.0));
    }

    #[test]
    fn markdown_highlighting_uses_theme_appropriate_colors() {
        let text = "# Heading\n`code` [link](https://example.com)\n- item";
        let light_palette = MarkdownPalette::new(
            Color32::from_rgb(42, 91, 166),
            Color32::from_rgb(35, 120, 70),
        );
        let dark_palette = MarkdownPalette::new(
            Color32::from_rgb(137, 190, 255),
            Color32::from_rgb(155, 226, 170),
        );
        let light = highlight(
            text,
            1.0,
            &FontFamily::Proportional,
            Color32::BLACK,
            light_palette,
            &[],
            &[],
        );
        let dark = highlight(
            text,
            1.0,
            &FontFamily::Proportional,
            Color32::WHITE,
            dark_palette,
            &[],
            &[],
        );

        let light_heading = light
            .sections
            .iter()
            .find(|section| section.format.color == light_palette.primary)
            .expect("light heading should use the theme primary color");
        let dark_heading = dark
            .sections
            .iter()
            .find(|section| section.format.color == dark_palette.primary)
            .expect("dark heading should use the theme primary color");

        assert!(light_heading.format.font_id.size == 16.0);
        assert!(dark_heading.format.font_id.size == 16.0);
    }

    #[test]
    fn markdown_list_highlighting_does_not_bleed_into_an_unmarked_line() {
        let text = "- item\nplain paragraph";
        let job = highlight(
            text,
            1.0,
            &FontFamily::Proportional,
            Color32::DARK_GRAY,
            palette(),
            &[],
            &[],
        );
        let list_color = palette().secondary;

        assert!(job.sections.iter().any(|section| {
            section.byte_range.start.0 < 6 && section.format.color == list_color
        }));
        assert!(job.sections.iter().any(|section| {
            section.byte_range.start.0 >= 7 && section.format.color == Color32::DARK_GRAY
        }));
    }
}
