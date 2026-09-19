use super::Theme;
use egui::Visuals;

pub fn apply_theme(ctx: &egui::Context, theme: &Theme) {
    let egui_theme = if theme.is_dark() {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_theme(egui_theme);
    let mut visuals = if egui_theme == egui::Theme::Dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let primary = theme.primary.0;
    let secondary = theme.secondary.0;
    let background = theme.background.0;
    visuals.panel_fill = background;
    visuals.window_fill = background;
    visuals.extreme_bg_color = background;
    visuals.faint_bg_color = secondary.gamma_multiply(0.13);
    visuals.code_bg_color = secondary.gamma_multiply(0.20);
    // visuals.selection.bg_fill = primary.gamma_multiply(0.50);
    visuals.window_stroke.color = theme.border_color();
    visuals.hyperlink_color = primary;
    visuals.widgets.inactive.bg_fill = secondary.gamma_multiply(0.20);
    visuals.widgets.hovered.bg_fill = secondary.gamma_multiply(0.42);
    visuals.widgets.active.bg_fill = primary.gamma_multiply(0.60);
    visuals.widgets.open.bg_fill = secondary.gamma_multiply(0.30);
    visuals.widgets.noninteractive.bg_stroke.color = theme.border_color();
    visuals.widgets.inactive.bg_stroke.color = theme.border_color();
    visuals.widgets.hovered.bg_stroke.color = theme.border_color();
    visuals.widgets.active.bg_stroke.color = theme.border_color();
    visuals.widgets.open.bg_stroke.color = theme.border_color();
    ctx.set_visuals(visuals);
    ctx.style_mut_of(egui_theme, |style| {
        style.spacing.scroll.dormant_handle_opacity = 0.1;
        style.spacing.scroll.active_handle_opacity = 0.1;
        style.spacing.scroll.interact_handle_opacity = 0.1;
        for font_id in style.text_styles.values_mut() {
            font_id.family = theme.system_font_family();
            font_id.size = theme.font_size;
        }
    });
}
