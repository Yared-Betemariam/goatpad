use eframe::egui;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Renders Markdown as a read-only document. The preview intentionally uses the
/// current content font and zoom so it feels like a locked version of the editor.
pub(super) fn render(
    ui: &mut egui::Ui,
    markdown: &str,
    zoom: f32,
    font_family: &egui::FontFamily,
    text_color: egui::Color32,
) {
    let mut job = egui::text::LayoutJob::default();
    let mut heading = None;
    let mut strong = false;
    let mut code_block = false;
    let mut list_depth = 0usize;
    let mut at_item_start = false;

    macro_rules! append {
        ($text:expr) => {{
            let size = match heading {
                Some(pulldown_cmark::HeadingLevel::H1) => 28.0,
                Some(pulldown_cmark::HeadingLevel::H2) => 23.0,
                Some(pulldown_cmark::HeadingLevel::H3) => 19.0,
                Some(_) => 17.0,
                None => 16.0,
            } * zoom;
            let mut format = egui::text::TextFormat::simple(
                egui::FontId::new(
                    size,
                    if code_block {
                        egui::FontFamily::Monospace
                    } else {
                        font_family.clone()
                    },
                ),
                text_color,
            );
            // The preview intentionally keeps the editor's normal colours;
            // its hierarchy is expressed through text sizing alone.
            if strong {
                format.font_id.size *= 1.08;
            }
            job.append($text, 0.0, format);
        }};
    }

    for event in Parser::new_ext(markdown, Options::all()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => heading = Some(level),
            Event::End(TagEnd::Heading(_)) => {
                heading = None;
                append!("\n\n");
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => append!("\n\n"),
            Event::Start(Tag::Strong) => strong = true,
            Event::End(TagEnd::Strong) => strong = false,

            Event::Start(Tag::CodeBlock(_)) => {
                code_block = true;
                append!("\n");
            }
            Event::End(TagEnd::CodeBlock) => {
                code_block = false;
                append!("\n\n");
            }
            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                append!("\n");
            }
            Event::Start(Tag::Item) => {
                at_item_start = true;
                append!("\n");
            }
            Event::End(TagEnd::Item) => at_item_start = false,
            Event::Text(text) | Event::Code(text) | Event::Html(text) | Event::InlineHtml(text) => {
                if at_item_start {
                    append!(&"  ".repeat(list_depth.saturating_sub(1)));
                    append!("• ");
                    at_item_start = false;
                }
                append!(&text);
            }
            Event::SoftBreak | Event::HardBreak => append!("\n"),
            Event::Rule => append!("\n──────────\n"),
            _ => {}
        }
    }

    if job.text.trim().is_empty() {
        append!("Nothing to preview.");
    }
    job.wrap.max_width = ui.available_width();
    ui.add(egui::Label::new(job).wrap());
}
