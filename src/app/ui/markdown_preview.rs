use eframe::egui;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Renders Markdown as a read-only document. The preview intentionally uses the
/// current content font and zoom so it feels like a locked version of the editor.
pub(super) fn render(
    ui: &mut egui::Ui,
    markdown: &str,
    zoom: f32,
    font_family: &egui::FontFamily,
    text_color: egui::Color32,
) {
    let mut job = layout(markdown, zoom, font_family, text_color);
    if job.text.trim().is_empty() {
        let mut placeholder = MarkdownRenderer::new(zoom, font_family, text_color);
        placeholder.append_text("Nothing to preview.");
        job = placeholder.job;
    }
    job.wrap.max_width = ui.available_width();
    ui.add(egui::Label::new(job).wrap());
}

struct ListState {
    ordered: bool,
    next_number: u64,
    item_open: bool,
    item_blocks: usize,
}

struct MarkdownRenderer<'a> {
    job: egui::text::LayoutJob,
    zoom: f32,
    font_family: &'a egui::FontFamily,
    text_color: egui::Color32,
    heading: Option<HeadingLevel>,
    strong_depth: usize,
    emphasis_depth: usize,
    strikethrough_depth: usize,
    link_depth: usize,
    code_block: bool,
    lists: Vec<ListState>,
    pending_breaks: usize,
    trailing_newlines: usize,
    has_content: bool,
}

impl<'a> MarkdownRenderer<'a> {
    fn new(zoom: f32, font_family: &'a egui::FontFamily, text_color: egui::Color32) -> Self {
        Self {
            job: egui::text::LayoutJob::default(),
            zoom,
            font_family,
            text_color,
            heading: None,
            strong_depth: 0,
            emphasis_depth: 0,
            strikethrough_depth: 0,
            link_depth: 0,
            code_block: false,
            lists: Vec::new(),
            pending_breaks: 0,
            trailing_newlines: 0,
            has_content: false,
        }
    }

    fn format(&self) -> egui::text::TextFormat {
        let size = match self.heading {
            Some(HeadingLevel::H1) => 28.0,
            Some(HeadingLevel::H2) => 23.0,
            Some(HeadingLevel::H3) => 19.0,
            Some(_) => 17.0,
            None => 16.0,
        } * self.zoom;
        let mut format = egui::text::TextFormat::simple(
            egui::FontId::new(
                size,
                if self.code_block {
                    egui::FontFamily::Monospace
                } else {
                    self.font_family.clone()
                },
            ),
            self.text_color,
        );
        if self.strong_depth > 0 {
            format.font_id.size *= 1.08;
        }
        if self.emphasis_depth > 0 {
            format.italics = true;
        }
        if self.strikethrough_depth > 0 {
            format.strikethrough = egui::Stroke::new(1.0, self.text_color);
        }
        if self.link_depth > 0 {
            format.underline = egui::Stroke::new(1.0, self.text_color);
        }
        format
    }

    fn marker_format(&self) -> egui::text::TextFormat {
        egui::text::TextFormat::simple(
            egui::FontId::new(self.zoom * 16.0, self.font_family.clone()),
            self.text_color,
        )
    }

    fn append_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let format = self.format();
        self.append_formatted(text, format);
    }

    fn append_marker(&mut self, text: &str) {
        self.append_formatted(text, self.marker_format());
    }

    fn append_formatted(&mut self, text: &str, format: egui::text::TextFormat) {
        if text.is_empty() {
            return;
        }
        self.flush_pending_breaks();
        self.job.append(text, 0.0, format);
        self.trailing_newlines = text
            .chars()
            .rev()
            .take_while(|character| *character == '\n')
            .count();
        self.has_content |= text.chars().any(|character| character != '\n');
    }

    fn append_unformatted_newlines(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        let format = self.format();
        let newlines = "\n".repeat(count);
        self.job.append(&newlines, 0.0, format);
        self.trailing_newlines += count;
    }

    fn flush_pending_breaks(&mut self) {
        if !self.has_content {
            self.pending_breaks = 0;
            return;
        }
        let missing = self.pending_breaks.saturating_sub(self.trailing_newlines);
        self.append_unformatted_newlines(missing);
        self.pending_breaks = 0;
    }

    /// Queues the minimum number of newlines needed before the next block.
    /// Delaying them avoids trailing blank lines and makes adjacent blocks use
    /// exactly one visual blank line regardless of the parser event sequence.
    fn request_breaks(&mut self, count: usize) {
        if self.has_content {
            self.pending_breaks = self.pending_breaks.max(count);
        }
    }

    fn request_line_break(&mut self) {
        if self.has_content {
            self.pending_breaks = self.pending_breaks.saturating_add(1);
        }
    }

    fn start_block(&mut self) {
        let item_block_count = self
            .lists
            .last()
            .filter(|list| list.item_open)
            .map(|list| list.item_blocks);
        if let Some(item_block_count) = item_block_count {
            if item_block_count > 0 {
                self.request_breaks(1);
            }
            if let Some(list) = self.lists.last_mut() {
                list.item_blocks += 1;
            }
            return;
        }
        self.request_breaks(2);
    }

    fn start_list(&mut self, start: Option<u64>) {
        if self.lists.is_empty() {
            self.request_breaks(2);
        } else {
            self.request_breaks(1);
        }
        self.lists.push(ListState {
            ordered: start.is_some(),
            next_number: start.unwrap_or(1),
            item_open: false,
            item_blocks: 0,
        });
    }

    fn start_item(&mut self) {
        let depth = self.lists.len();
        let (ordered, number) = {
            let list = self
                .lists
                .last_mut()
                .expect("Markdown list items always belong to a list");
            let number = list.next_number;
            if list.ordered {
                list.next_number = list.next_number.saturating_add(1);
            }
            list.item_open = true;
            list.item_blocks = 0;
            (list.ordered, number)
        };
        let marker = if ordered {
            format!("{}{}.", "  ".repeat(depth.saturating_sub(1)), number)
        } else {
            format!("{}•", "  ".repeat(depth.saturating_sub(1)))
        };
        self.append_marker(&format!("{marker} "));
    }

    fn end_item(&mut self) {
        if let Some(list) = self.lists.last_mut() {
            list.item_open = false;
        }
        self.request_breaks(1);
    }

    fn finish(self) -> egui::text::LayoutJob {
        self.job
    }
}

fn layout(
    markdown: &str,
    zoom: f32,
    font_family: &egui::FontFamily,
    text_color: egui::Color32,
) -> egui::text::LayoutJob {
    let mut renderer = MarkdownRenderer::new(zoom, font_family, text_color);

    for event in Parser::new_ext(markdown, Options::all()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                renderer.start_block();
                renderer.heading = Some(level);
            }
            Event::End(TagEnd::Heading(_)) => renderer.heading = None,

            Event::Start(Tag::Paragraph) => renderer.start_block(),
            Event::End(TagEnd::Paragraph) => {}

            Event::Start(Tag::Strong) => renderer.strong_depth += 1,
            Event::End(TagEnd::Strong) => {
                renderer.strong_depth = renderer.strong_depth.saturating_sub(1)
            }
            Event::Start(Tag::Emphasis) => renderer.emphasis_depth += 1,
            Event::End(TagEnd::Emphasis) => {
                renderer.emphasis_depth = renderer.emphasis_depth.saturating_sub(1)
            }
            Event::Start(Tag::Strikethrough) => renderer.strikethrough_depth += 1,
            Event::End(TagEnd::Strikethrough) => {
                renderer.strikethrough_depth = renderer.strikethrough_depth.saturating_sub(1)
            }
            Event::Start(Tag::Link { .. }) => renderer.link_depth += 1,
            Event::End(TagEnd::Link) => renderer.link_depth = renderer.link_depth.saturating_sub(1),

            Event::Start(Tag::CodeBlock(_)) => {
                renderer.start_block();
                renderer.code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                renderer.code_block = false;
            }

            Event::Start(Tag::List(start)) => renderer.start_list(start),
            Event::End(TagEnd::List(_)) => {
                renderer.lists.pop();
            }
            Event::Start(Tag::Item) => renderer.start_item(),
            Event::End(TagEnd::Item) => renderer.end_item(),

            Event::TaskListMarker(checked) => {
                renderer.append_marker(if checked { "☑ " } else { "☐ " });
            }
            Event::Code(text) => {
                let previous_code_block = renderer.code_block;
                renderer.code_block = true;
                renderer.append_text(&text);
                renderer.code_block = previous_code_block;
            }
            Event::Text(text) => {
                let text: &str = &text;
                if renderer.code_block {
                    let text = text
                        .strip_suffix("\r\n")
                        .or_else(|| text.strip_suffix('\n'))
                        .unwrap_or(text);
                    renderer.append_text(text);
                } else {
                    renderer.append_text(text);
                }
            }
            Event::Html(text) | Event::InlineHtml(text) => renderer.append_text(&text),
            Event::SoftBreak | Event::HardBreak => renderer.request_line_break(),
            Event::Rule => {
                renderer.start_block();
                renderer.append_text("──────────");
            }
            _ => {}
        }
    }

    renderer.finish()
}

#[cfg(test)]
mod tests {
    use super::layout;
    use egui::{Color32, FontFamily};

    fn rendered(markdown: &str) -> String {
        layout(markdown, 1.0, &FontFamily::Proportional, Color32::BLACK).text
    }

    #[test]
    fn blocks_have_one_blank_line_without_trailing_spacing() {
        assert_eq!(
            rendered("# Heading\n\nParagraph\n\n## Next"),
            "Heading\n\nParagraph\n\nNext"
        );
    }

    #[test]
    fn ordered_lists_render_start_number_and_increment_items() {
        assert_eq!(
            rendered("3. First\n4. Second\n5. Third"),
            "3. First\n4. Second\n5. Third"
        );
    }

    #[test]
    fn nested_lists_are_indented_and_do_not_add_blank_lines_between_items() {
        assert_eq!(
            rendered("- Parent\n  - Child\n  - Another child\n- Sibling"),
            "• Parent\n  • Child\n  • Another child\n• Sibling"
        );
    }

    #[test]
    fn inline_styles_do_not_change_block_spacing() {
        assert_eq!(
            rendered("**strong** and *emphasis* with ~~strike~~\n\n[link](https://example.com)"),
            "strong and emphasis with strike\n\nlink"
        );
    }

    #[test]
    fn code_blocks_do_not_leave_a_structural_trailing_line() {
        assert_eq!(
            rendered("Before\n\n```text\nlet value = 1;\n```\n\nAfter"),
            "Before\n\nlet value = 1;\n\nAfter"
        );
    }
}
