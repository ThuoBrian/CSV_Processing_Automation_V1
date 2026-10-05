//! Workflow 1: clean a single printer CSV. A direct state-machine-free port
//! of the old `process_single_file()` — pick input, optionally override the
//! output path, process, preview.

use std::path::PathBuf;

use eframe::egui;
use polars::prelude::{DataFrame, PolarsError};

use csv_processing_automation::{output_file_name, process_csv_file};

use super::style::{self, Status};
use super::widgets;

pub struct State {
    selected_input: Option<PathBuf>,
    input_dir: PathBuf,
    output_dir: PathBuf,
    auto_generate_output: bool,
    custom_output_path: String,
    result: Option<Result<DataFrame, PolarsError>>,
    show_preview: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            selected_input: None,
            input_dir: PathBuf::from("./data"),
            output_dir: PathBuf::from("./output"),
            auto_generate_output: true,
            custom_output_path: String::new(),
            result: None,
            show_preview: false,
        }
    }
}

/// Render the single-file workflow screen. Returns `true` when the user
/// wants to go back to the menu.
pub fn ui(ui: &mut egui::Ui, state: &mut State) -> bool {
    ui.heading("Clean a single printer CSV");
    let back = style::back_button(ui, "\u{2190} Back to menu");
    ui.add_space(8.0);

    style::card(ui, |ui| {
        widgets::directory_picker(
            ui,
            "single_file_input_dir",
            "Input folder:",
            &mut state.input_dir,
        );
        let files = widgets::list_files_with_extension(&state.input_dir, "csv");
        if files.is_empty() {
            style::banner(
                ui,
                Status::Error,
                &format!("No .csv files found in {}", state.input_dir.display()),
            );
        }
        widgets::file_picker(
            ui,
            "single_file_input",
            "Input CSV:",
            &files,
            &mut state.selected_input,
        );
    });

    ui.add_space(style::SECTION_SPACING);
    let output_path: Option<PathBuf> = style::card(ui, |ui| {
        ui.checkbox(
            &mut state.auto_generate_output,
            "Generate output path automatically",
        );

        if state.auto_generate_output {
            widgets::directory_picker(
                ui,
                "single_file_output_dir",
                "Output folder:",
                &mut state.output_dir,
            );
            state.selected_input.as_ref().map(|p| {
                std::fs::create_dir_all(&state.output_dir).ok();
                state.output_dir.join(output_file_name(p))
            })
        } else {
            if let Some(input) = &state.selected_input {
                if state.custom_output_path.is_empty() {
                    state.custom_output_path = state
                        .output_dir
                        .join(output_file_name(input))
                        .to_string_lossy()
                        .to_string();
                }
            }
            ui.text_edit_singleline(&mut state.custom_output_path);
            let trimmed = state.custom_output_path.trim_matches('"');
            if trimmed.is_empty() {
                None
            } else {
                Some(PathBuf::from(trimmed))
            }
        }
    });

    ui.add_space(style::SECTION_SPACING);
    let can_process = state.selected_input.is_some() && output_path.is_some();
    if ui
        .add_enabled(can_process, style::primary_button_widget("Process"))
        .clicked()
    {
        if let (Some(input), Some(output)) = (&state.selected_input, &output_path) {
            state.result = Some(process_csv_file(input, output));
            state.show_preview = false;
        }
    }

    ui.add_space(style::SECTION_SPACING);
    match &state.result {
        Some(Ok(df)) => {
            style::card(ui, |ui| {
                style::banner(
                    ui,
                    Status::Success,
                    &format!("Success — processed {} rows", df.height()),
                );
                ui.checkbox(&mut state.show_preview, "Preview first 5 rows");
                if state.show_preview {
                    widgets::dataframe_preview(ui, df, 5);
                }
                if let Some(dir) = output_path.as_ref().and_then(|p| p.parent()) {
                    widgets::open_output_folder_button(ui, dir);
                }
            });
        }
        Some(Err(e)) => {
            style::card(ui, |ui| {
                style::banner(ui, Status::Error, &format!("Failed to process CSV: {e}"));
            });
        }
        None => {}
    }

    back
}
