use eframe::egui;

const APP_ICON_SIZE: usize = 64;
const APP_ICON_RGBA: &[u8; APP_ICON_SIZE * APP_ICON_SIZE * 4] =
    include_bytes!("../../assets/icon.rgba");

pub(crate) fn app_icon() -> egui::IconData {
    egui::IconData {
        rgba: APP_ICON_RGBA.to_vec(),
        width: APP_ICON_SIZE as u32,
        height: APP_ICON_SIZE as u32,
    }
}

pub(super) fn load_app_icon_texture(ctx: &egui::Context) -> egui::TextureHandle {
    ctx.load_texture(
        "goatpad-app-icon",
        egui::ColorImage::from_rgba_unmultiplied([APP_ICON_SIZE, APP_ICON_SIZE], APP_ICON_RGBA),
        egui::TextureOptions::LINEAR,
    )
}
