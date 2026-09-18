use egui::{
    Color32, FontFamily, FontId, Stroke,
    text::{LayoutJob, TextFormat},
};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    Heading,
    Strong,
    Emphasis,
    Code,
    Link,
    List,
}

impl Style {
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

    fn format(self, zoom: f32, font_family: &FontFamily, text_color: Color32) -> TextFormat {
        let mut format = default_format(zoom, font_family, text_color);
        match self {
            Self::Heading => {
                format.font_id = FontId::new(20.0 * zoom, font_family.clone());
                format.color = Color32::from_rgb(111, 168, 255);
            }
            Self::Strong => format.font_id = FontId::new(16.0 * zoom, font_family.clone()),
            Self::Emphasis => format.italics = true,
            Self::Code => {
                format.font_id = FontId::new(15.0 * zoom, FontFamily::Monospace);
                format.color = Color32::from_rgb(232, 177, 94);
                format.background = Color32::from_rgb(45, 45, 50);
            }
            Self::Link => {
                format.color = Color32::from_rgb(97, 183, 255);
                format.underline = Stroke::new(1.0, format.color);
            }
            Self::List => format.color = Color32::from_rgb(132, 205, 150),
        }
        format
    }
}

fn default_format(zoom: f32, font_family: &FontFamily, text_color: Color32) -> TextFormat {
    TextFormat::simple(FontId::new(16.0 * zoom, font_family.clone()), text_color)
}

/// The color used to underline misspelled words, matching Notepad's spell-check styling.
pub const MISSPELLED_UNDERLINE_COLOR: Color32 = Color32::from_rgb(224, 49, 49);

/// Marks `format` as misspelled by drawing a red underline, preserving every
/// other attribute (color, weight, background, …) it already carries.
fn mark_misspelled(mut format: TextFormat, zoom: f32) -> TextFormat {
    format.underline = Stroke::new((1.3 * zoom).max(1.0), MISSPELLED_UNDERLINE_COLOR);
    format
}

/// Produces a live Markdown layout while preserving the editor's original text.
/// `zoom` scales every font size uniformly, mirroring Notepad's zoom control.
/// `misspelled` lists the UTF-8 byte ranges that should be underlined in red.
pub fn highlight(
    text: &str,
    zoom: f32,
    font_family: &FontFamily,
    text_color: Color32,
    misspelled: &[Range<usize>],
) -> LayoutJob {
    let mut spans = Vec::<(Range<usize>, Style)>::new();
    let mut active = Vec::<Style>::new();

    for (event, range) in Parser::new_ext(text, Options::all()).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if let Some(style) = style_for_tag(&tag) {
                    active.push(style);
                    spans.push((range, style));
                }
            }
            Event::End(tag_end) => {
                if let Some(style) = style_for_end(tag_end) {
                    spans.push((range, style));
                    if let Some(index) = active.iter().rposition(|current| *current == style) {
                        active.remove(index);
                    }
                }
            }
            Event::Code(_) => spans.push((range, Style::Code)),
            Event::Text(_) | Event::Html(_) | Event::InlineHtml(_) => {
                if let Some(style) = active.iter().max_by_key(|style| style.priority()) {
                    spans.push((range, *style));
                }
            }
            _ => {}
        }
    }

    layout_with_spans(text, spans, zoom, font_family, text_color, misspelled)
}

/// Produces a plain-text layout, underlining misspelled words (given as
/// UTF-8 byte ranges) in red.
pub fn plain(
    text: &str,
    zoom: f32,
    font_family: &FontFamily,
    text_color: Color32,
    misspelled: &[Range<usize>],
) -> LayoutJob {
    let format = default_format(zoom, font_family, text_color);
    if misspelled.is_empty() {
        return LayoutJob::simple(text.to_owned(), format.font_id, format.color, f32::INFINITY);
    }

    let flags = misspelled_flags(text.len(), misspelled);
    let mut job = LayoutJob::default();
    let mut start = 0;
    let mut current = flags.first().copied().unwrap_or(false);
    for (index, _) in text.char_indices().skip(1) {
        if flags[index] != current {
            job.append(
                &text[start..index],
                0.0,
                if current {
                    mark_misspelled(format.clone(), zoom)
                } else {
                    format.clone()
                },
            );
            start = index;
            current = flags[index];
        }
    }
    if !text.is_empty() {
        job.append(
            &text[start..],
            0.0,
            if current {
                mark_misspelled(format.clone(), zoom)
            } else {
                format
            },
        );
    }
    job
}

fn style_for_tag(tag: &Tag<'_>) -> Option<Style> {
    match tag {
        Tag::Heading { .. } => Some(Style::Heading),
        Tag::Strong => Some(Style::Strong),
        Tag::Emphasis => Some(Style::Emphasis),
        Tag::Link { .. } => Some(Style::Link),
        Tag::Item => Some(Style::List),
        _ => None,
    }
}

fn style_for_end(tag: TagEnd) -> Option<Style> {
    match tag {
        TagEnd::Heading(_) => Some(Style::Heading),
        TagEnd::Strong => Some(Style::Strong),
        TagEnd::Emphasis => Some(Style::Emphasis),
        TagEnd::Link => Some(Style::Link),
        TagEnd::Item => Some(Style::List),
        _ => None,
    }
}

fn layout_with_spans(
    text: &str,
    spans: Vec<(Range<usize>, Style)>,
    zoom: f32,
    font_family: &FontFamily,
    text_color: Color32,
    misspelled: &[Range<usize>],
) -> LayoutJob {
    let mut styles = vec![None; text.len()];
    for (range, style) in spans {
        for current_style in styles
            .iter_mut()
            .take(range.end.min(text.len()))
            .skip(range.start.min(text.len()))
        {
            if current_style.is_none_or(|current: Style| style.priority() >= current.priority()) {
                *current_style = Some(style);
            }
        }
    }
    let flags = misspelled_flags(text.len(), misspelled);

    let format_at = |style: Option<Style>, is_misspelled: bool| {
        let format = style.map_or_else(
            || default_format(zoom, font_family, text_color),
            |style| style.format(zoom, font_family, text_color),
        );
        if is_misspelled {
            mark_misspelled(format, zoom)
        } else {
            format
        }
    };
    let mut job = LayoutJob::default();
    let mut start = 0;
    let mut current = (
        styles.first().copied().flatten(),
        flags.first().copied().unwrap_or(false),
    );
    for (index, _) in text.char_indices().skip(1) {
        let key = (styles[index], flags[index]);
        if key != current {
            job.append(&text[start..index], 0.0, format_at(current.0, current.1));
            start = index;
            current = key;
        }
    }
    if !text.is_empty() {
        job.append(&text[start..], 0.0, format_at(current.0, current.1));
    }
    job
}

/// Builds a per-byte boolean mask marking which positions in a `len`-byte
/// text fall inside one of the given misspelled-word ranges.
fn misspelled_flags(len: usize, misspelled: &[Range<usize>]) -> Vec<bool> {
    let mut flags = vec![false; len];
    for range in misspelled {
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
    use super::{highlight, plain};
    use egui::{Color32, FontFamily};

    #[test]
    fn highlighting_covers_the_entire_document() {
        let job = highlight(
            "# Heading\n\nA **bold** [link](https://example.com).",
            1.0,
            &FontFamily::Proportional,
            Color32::DARK_GRAY,
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
            &[2..12],
        );

        let misspelled_section = job
            .sections
            .iter()
            .find(|section| section.format.underline.width > 0.0)
            .expect("expected a misspelled section to be underlined");
        // The word is still bold (larger font), and now also underlined in red.
        assert!(misspelled_section.format.font_id.size == 16.0);
        assert_eq!(
            misspelled_section.format.underline.color,
            super::MISSPELLED_UNDERLINE_COLOR
        );
    }

    #[test]
    fn markdown_constructs_receive_distinct_formats() {
        let job = highlight(
            "# Heading\n*italic* **bold** `code` [link](https://example.com)\n- item",
            1.0,
            &FontFamily::Proportional,
            Color32::DARK_GRAY,
            &[],
        );
        let formats = job
            .sections
            .iter()
            .map(|section| &section.format)
            .collect::<Vec<_>>();

        assert!(formats.iter().any(|format| format.font_id.size >= 20.0));
        assert!(formats.iter().any(|format| format.italics));
        assert!(formats.iter().any(|format| format.font_id.size == 16.0));
        assert!(
            formats
                .iter()
                .any(|format| format.font_id.family == egui::FontFamily::Monospace)
        );
        assert!(formats.iter().any(|format| format.underline.width > 0.0));
    }
}
