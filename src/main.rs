use eframe::egui;

#[derive(Default)]
struct GoatpadApp;

impl eframe::App for GoatpadApp {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("Goatpad"),
        ..Default::default()
    };

    eframe::run_native(
        "Goatpad",
        options,
        Box::new(|_creation_context| Ok(Box::new(GoatpadApp))),
    )
}
