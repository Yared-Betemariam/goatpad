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

    fn format(self) -> TextFormat {
        let mut format = default_format();
        match self {
            Self::Heading => {
                format.font_id = FontId::new(20.0, FontFamily::Proportional);
                format.color = Color32::from_rgb(111, 168, 255);
            }
            Self::Strong => format.font_id = FontId::new(16.0, FontFamily::Proportional),
            Self::Emphasis => format.italics = true,
            Self::Code => {
                format.font_id = FontId::new(15.0, FontFamily::Monospace);
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

fn default_format() -> TextFormat {
    TextFormat::simple(
        FontId::new(16.0, FontFamily::Proportional),
        Color32::LIGHT_GRAY,
    )
}

/// Produces a live Markdown layout while preserving the editor's original text.
pub fn highlight(text: &str) -> LayoutJob {
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

    layout_with_spans(text, spans)
}

pub fn plain(text: &str) -> LayoutJob {
    LayoutJob::simple(
        text.to_owned(),
        default_format().font_id,
        default_format().color,
        f32::INFINITY,
    )
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

fn layout_with_spans(text: &str, spans: Vec<(Range<usize>, Style)>) -> LayoutJob {
    let mut styles = vec![None; text.len()];
    for (range, style) in spans {
        for byte in range.start.min(text.len())..range.end.min(text.len()) {
            if styles[byte].is_none_or(|current: Style| style.priority() >= current.priority()) {
                styles[byte] = Some(style);
            }
        }
    }

    let mut job = LayoutJob::default();
    let mut start = 0;
    let mut current = styles.first().copied().flatten();
    for (index, _) in text.char_indices().skip(1) {
        if styles[index] != current {
            job.append(
                &text[start..index],
                0.0,
                current.map_or_else(default_format, Style::format),
            );
            start = index;
            current = styles[index];
        }
    }
    if !text.is_empty() {
        job.append(
            &text[start..],
            0.0,
            current.map_or_else(default_format, Style::format),
        );
    }
    job
}

#[cfg(test)]
mod tests {
    use super::highlight;

    #[test]
    fn highlighting_covers_the_entire_document() {
        let job = highlight("# Heading\n\nA **bold** [link](https://example.com).");
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
    fn markdown_constructs_receive_distinct_formats() {
        let job =
            highlight("# Heading\n*italic* **bold** `code` [link](https://example.com)\n- item");
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
