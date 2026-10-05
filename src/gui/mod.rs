mod cmr_matching;
mod single_file;
pub mod style;
mod widgets;

use eframe::egui;

/// Top-level application: a tiny menu dispatching to one of two workflow
/// screens, each driven by its own local state.
pub struct CsvApp {
    screen: Screen,
}

enum Screen {
    Menu,
    SingleFile(single_file::State),
    // Boxed: CmrMatching::State is much larger than the other variants
    // (clippy::large_enum_variant).
    CmrMatching(Box<cmr_matching::State>),
}

impl Default for CsvApp {
    fn default() -> Self {
        Self {
            screen: Screen::Menu,
        }
    }
}

impl eframe::App for CsvApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match &mut self.screen {
                    Screen::Menu => {
                        ui.add_space(style::SECTION_SPACING);
                        ui.vertical_centered(|ui| {
                            ui.heading("PrintLedger");
                            ui.weak("CSV Processing Automation for printer usage & CMR reports");
                        });
                        ui.add_space(style::SECTION_SPACING);
                        ui.separator();
                        ui.add_space(style::SECTION_SPACING);

                        style::card(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.strong("Clean a single printer CSV");
                                ui.weak("Validate and reformat one printer-usage export.");
                                ui.add_space(6.0);
                                if ui
                                    .add_sized(
                                        [ui.available_width().min(260.0), 36.0],
                                        style::primary_button_widget("Clean a CSV"),
                                    )
                                    .clicked()
                                {
                                    self.screen = Screen::SingleFile(single_file::State::default());
                                }
                            });
                        });
                        ui.add_space(12.0);
                        style::card(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.strong("Match readings to CMR report");
                                ui.weak(
                                    "Match printer readings against the source_of_truth workbook.",
                                );
                                ui.add_space(6.0);
                                if ui
                                    .add_sized(
                                        [ui.available_width().min(260.0), 36.0],
                                        style::primary_button_widget("Start matching"),
                                    )
                                    .clicked()
                                {
                                    self.screen = Screen::CmrMatching(Box::default());
                                }
                            });
                        });
                    }
                    Screen::SingleFile(state) => {
                        if single_file::ui(ui, state) {
                            self.screen = Screen::Menu;
                        }
                    }
                    Screen::CmrMatching(state) => {
                        if cmr_matching::ui(ui, state) {
                            self.screen = Screen::Menu;
                        }
                    }
                });
        });
    }
}
