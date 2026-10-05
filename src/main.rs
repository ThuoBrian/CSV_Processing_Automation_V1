// Hide the console window; this is a GUI application.
#![windows_subsystem = "windows"]

mod gui;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("PrintLedger")
            .with_inner_size([720.0, 560.0])
            .with_min_inner_size([480.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PrintLedger",
        options,
        Box::new(|cc| {
            gui::style::apply(&cc.egui_ctx);
            Ok(Box::new(gui::CsvApp::default()))
        }),
    )
}
