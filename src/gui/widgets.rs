//! Shared helpers used by both workflow screens: directory-backed file
//! pickers, a plain string picker, a DataFrame preview table, and an
//! "open output folder" button.

use std::path::{Path, PathBuf};

use eframe::egui;
use polars::prelude::DataFrame;

/// List files in `dir` matching `extension` (e.g. "csv", "xlsx"), sorted by
/// name. Ported verbatim from the previous `inquire`-based `main.rs` — it
/// never depended on `inquire`.
pub fn list_files_with_extension(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case(extension))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// Render a combo box that picks one of `files` by file name. The selection
/// is always one of the listed paths (never free-typed text), so there is
/// no path-traversal surface here.
pub fn file_picker(
    ui: &mut egui::Ui,
    id_salt: &str,
    label: &str,
    files: &[PathBuf],
    selected: &mut Option<PathBuf>,
) {
    ui.label(label);
    let selected_label = selected
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "(choose a file)".to_string());

    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(selected_label)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for file in files {
                let name = file
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let is_selected = selected.as_deref() == Some(file.as_path());
                if ui.selectable_label(is_selected, name).clicked() {
                    *selected = Some(file.clone());
                }
            }
        });
}

/// Render a combo box that picks one of `options` (plain strings, e.g. sheet
/// names, block names, or column names).
pub fn string_picker(
    ui: &mut egui::Ui,
    id_salt: &str,
    label: &str,
    options: &[String],
    selected: &mut Option<String>,
) {
    ui.label(label);
    let selected_label = selected.clone().unwrap_or_else(|| "(choose)".to_string());

    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(selected_label)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for option in options {
                let is_selected = selected.as_deref() == Some(option.as_str());
                if ui.selectable_label(is_selected, option).clicked() {
                    *selected = Some(option.clone());
                }
            }
        });
}

/// Render a preview table of the first `n_rows` rows of `df`.
pub fn dataframe_preview(ui: &mut egui::Ui, df: &DataFrame, n_rows: usize) {
    let column_names: Vec<String> = df
        .get_column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let n_rows = n_rows.min(df.height());

    egui_extras::TableBuilder::new(ui)
        .striped(true)
        .columns(
            egui_extras::Column::auto().resizable(true),
            column_names.len(),
        )
        .header(20.0, |mut header| {
            for name in &column_names {
                header.col(|ui| {
                    ui.strong(name);
                });
            }
        })
        .body(|mut body| {
            for row_idx in 0..n_rows {
                body.row(18.0, |mut row| {
                    for name in &column_names {
                        row.col(|ui| {
                            let text = df
                                .column(name)
                                .ok()
                                .and_then(|c| c.get(row_idx).ok())
                                .map(|v| format!("{v}"))
                                .unwrap_or_default();
                            ui.label(text);
                        });
                    }
                });
            }
        });
}

/// Render an "Open output folder" button that opens `dir` in the OS file
/// explorer when clicked. The click itself is the user's confirmation —
/// matches the previous `inquire::Confirm` behavior, no extra prompt.
pub fn open_output_folder_button(ui: &mut egui::Ui, dir: &std::path::Path) {
    if ui.button("Open output folder").clicked() {
        // Best-effort: if the OS can't launch a file explorer, there's
        // nothing actionable for the user to do about it here.
        let _ = open::that(dir);
    }
}

/// Render an editable path field plus a "Browse..." button that opens a
/// native folder-picker dialog, defaulting to `dir`'s current value.
pub fn directory_picker(ui: &mut egui::Ui, id_salt: &str, label: &str, dir: &mut PathBuf) {
    ui.label(label);
    ui.horizontal(|ui| {
        const BROWSE_WIDTH: f32 = 84.0;
        let row_height = ui.spacing().interact_size.y;
        let spacing = ui.spacing().item_spacing.x;
        let text_width = (ui.available_width() - BROWSE_WIDTH - spacing).max(80.0);

        let mut text = dir.to_string_lossy().to_string();
        if ui
            .add_sized(
                [text_width, row_height],
                egui::TextEdit::singleline(&mut text).id_salt(id_salt),
            )
            .changed()
        {
            *dir = PathBuf::from(text);
        }
        if ui
            .add_sized([BROWSE_WIDTH, row_height], egui::Button::new("Browse..."))
            .clicked()
        {
            let mut dialog = rfd::FileDialog::new();
            if dir.is_dir() {
                dialog = dialog.set_directory(&dir);
            }
            if let Some(picked) = dialog.pick_folder() {
                *dir = picked;
            }
        }
    });
}
