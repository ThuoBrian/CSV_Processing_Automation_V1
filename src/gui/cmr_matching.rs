//! Workflow 2: match printer readings to the source_of_truth workbook and
//! produce a CMR report. A step-based state machine that is a direct port
//! of the old `process_cmr_matching()` — only the interaction mechanism
//! (egui steps instead of `inquire` prompts) changed, not the matching or
//! resolution logic.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use calamine::Xlsx;
use eframe::egui;
use polars::prelude::DataFrame;

use csv_processing_automation::matcher::{match_candidates, MatchCandidate, MatchMethod};
use csv_processing_automation::printer_codes::{build_lookup, parse_codes_sheet, CodesSheet};
use csv_processing_automation::read_full_csv;
use csv_processing_automation::report::{
    write_paste_ready_report, write_review_report, MatchedRow, ProjectCmr,
};
use csv_processing_automation::source_of_truth::{
    list_sheets, load_workbook, parse_sheet, Block, SheetData,
};

use super::style::{self, Status};
use super::widgets;

type Workbook = Xlsx<BufReader<File>>;

enum Step {
    PickDataCsv,
    AskUseCodesLookup,
    PickCodesWorkbook,
    PickCodesSheet,
    PickCodesIdNameColumns,
    PickSourceOfTruthWorkbook,
    PickSourceOfTruthSheet,
    PickBlock,
    PickPrintCopyColumns,
    MatchingRow,
    Done,
}

/// `(current_index, total_wizard_steps, human_label)` for the 9 picker
/// steps — a fixed nominal position in the wizard's maximum path (steps
/// 2-5, the codes lookup, are conditionally skipped when there's no `User`
/// column, so this numbering doesn't dynamically renumber). Presentation
/// only; does not affect `Step`'s variants or transitions. `None` for
/// `MatchingRow`/`Done`, which have their own distinct progress UI.
fn step_label(step: &Step) -> Option<(usize, usize, &'static str)> {
    const TOTAL: usize = 9;
    match step {
        Step::PickDataCsv => Some((1, TOTAL, "Printer usage CSV")),
        Step::AskUseCodesLookup => Some((2, TOTAL, "Printer-codes lookup?")),
        Step::PickCodesWorkbook => Some((3, TOTAL, "Codes workbook")),
        Step::PickCodesSheet => Some((4, TOTAL, "Codes sheet")),
        Step::PickCodesIdNameColumns => Some((5, TOTAL, "Codes ID / name columns")),
        Step::PickSourceOfTruthWorkbook => Some((6, TOTAL, "CMR tracker workbook")),
        Step::PickSourceOfTruthSheet => Some((7, TOTAL, "CMR tracker sheet")),
        Step::PickBlock => Some((8, TOTAL, "Office / printer block")),
        Step::PickPrintCopyColumns => Some((9, TOTAL, "Print / copy columns & output")),
        Step::MatchingRow | Step::Done => None,
    }
}

/// An ambiguous match awaiting user confirmation for the row currently at
/// `current_row`. Computed once when a row needs a pause, then held here
/// until the "Confirm" button is clicked.
struct PendingMatch {
    data_name: String,
    resolved_via: &'static str,
    cmr_print_value: String,
    cmr_copy_value: String,
    candidates: Vec<MatchCandidate>,
    /// Index into `candidates`, or `candidates.len()` for "No match / skip".
    choice: usize,
}

pub struct State {
    step: Step,

    // Directory roots, user-overridable via a folder-picker; default to the
    // previously hardcoded paths.
    data_dir: PathBuf,
    source_of_truth_dir: PathBuf,
    output_dir: PathBuf,

    // Step: pick the data CSV.
    data_path: Option<PathBuf>,
    df: Option<DataFrame>,
    column_names: Vec<String>,
    has_user_col: bool,

    // Step: optional printer-codes lookup.
    use_codes_lookup: Option<bool>,
    codes_lookup: HashMap<String, String>,
    codes_loaded_count: Option<usize>,
    codes_workbook_path: Option<PathBuf>,
    codes_workbook: Option<Workbook>,
    codes_sheet_names: Vec<String>,
    codes_sheet_name: Option<String>,
    codes_sheet: Option<CodesSheet>,
    codes_id_col: Option<String>,
    codes_name_col: Option<String>,

    // Step: source_of_truth workbook + sheet.
    workbook_path: Option<PathBuf>,
    workbook: Option<Workbook>,
    sheet_names: Vec<String>,
    sheet_name: Option<String>,
    sheet_data: Option<SheetData>,

    // Step: which office/printer block.
    block_choice: Option<String>,
    block: Option<Block>,

    // Step: which columns feed CMR(print)/CMR(copy).
    print_col: Option<String>,
    copy_col: Option<String>,

    // Matching loop.
    current_row: usize,
    pending_candidates: Option<PendingMatch>,
    rows: Vec<MatchedRow>,
    readings_by_project: HashMap<usize, (String, String)>,
    auto_matched: usize,
    confirmed_matched: usize,
    skipped: usize,
    warnings: Vec<String>,

    // Final reports.
    paste_ready_path: Option<PathBuf>,
    review_path: Option<PathBuf>,

    error: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            step: Step::PickDataCsv,
            data_dir: PathBuf::from("./data"),
            source_of_truth_dir: PathBuf::from("./source_of_truth"),
            output_dir: PathBuf::from("./output"),
            data_path: None,
            df: None,
            column_names: Vec::new(),
            has_user_col: false,
            use_codes_lookup: None,
            codes_lookup: HashMap::new(),
            codes_loaded_count: None,
            codes_workbook_path: None,
            codes_workbook: None,
            codes_sheet_names: Vec::new(),
            codes_sheet_name: None,
            codes_sheet: None,
            codes_id_col: None,
            codes_name_col: None,
            workbook_path: None,
            workbook: None,
            sheet_names: Vec::new(),
            sheet_name: None,
            sheet_data: None,
            block_choice: None,
            block: None,
            print_col: None,
            copy_col: None,
            current_row: 0,
            pending_candidates: None,
            rows: Vec::new(),
            readings_by_project: HashMap::new(),
            auto_matched: 0,
            confirmed_matched: 0,
            skipped: 0,
            warnings: Vec::new(),
            paste_ready_path: None,
            review_path: None,
            error: None,
        }
    }
}

/// Render the CMR-matching workflow screen. Returns `true` when the user
/// wants to go back to the menu.
pub fn ui(ui: &mut egui::Ui, state: &mut State) -> bool {
    ui.heading("Match printer readings to source_of_truth (CMR report)");
    let back = style::back_button(ui, "\u{2190} Back to menu");
    ui.add_space(8.0);

    if let Some((idx, total, label)) = step_label(&state.step) {
        ui.add(
            egui::ProgressBar::new(idx as f32 / total as f32)
                .text(format!("Step {idx} of {total} — {label}")),
        );
        ui.add_space(style::SECTION_SPACING);
    }

    if let Some(err) = state.error.clone() {
        style::banner(ui, Status::Error, &err);
        ui.add_space(8.0);
    }

    match state.step {
        Step::PickDataCsv => render_pick_data_csv(ui, state),
        Step::AskUseCodesLookup => render_ask_use_codes_lookup(ui, state),
        Step::PickCodesWorkbook => render_pick_codes_workbook(ui, state),
        Step::PickCodesSheet => render_pick_codes_sheet(ui, state),
        Step::PickCodesIdNameColumns => render_pick_codes_id_name_columns(ui, state),
        Step::PickSourceOfTruthWorkbook => render_pick_source_of_truth_workbook(ui, state),
        Step::PickSourceOfTruthSheet => render_pick_source_of_truth_sheet(ui, state),
        Step::PickBlock => render_pick_block(ui, state),
        Step::PickPrintCopyColumns => render_pick_print_copy_columns(ui, state),
        Step::MatchingRow => {
            if state.pending_candidates.is_none() {
                advance_matching(state);
            }
            match state.step {
                Step::Done => render_done(ui, state),
                Step::MatchingRow => render_matching_row(ui, state),
                _ => unreachable!("advance_matching only moves to MatchingRow's pause or Done"),
            }
        }
        Step::Done => render_done(ui, state),
    }

    back
}

fn render_pick_data_csv(ui: &mut egui::Ui, state: &mut State) {
    ui.strong("Printer usage CSV");
    let next_clicked = style::card(ui, |ui| {
        widgets::directory_picker(ui, "cmr_data_dir", "Data folder:", &mut state.data_dir);
        let files = widgets::list_files_with_extension(&state.data_dir, "csv");
        if files.is_empty() {
            style::banner(
                ui,
                Status::Error,
                &format!("No .csv files found in {}", state.data_dir.display()),
            );
        }
        widgets::file_picker(
            ui,
            "cmr_data_csv",
            "Printer usage CSV:",
            &files,
            &mut state.data_path,
        );

        style::primary_button(ui, "Next").clicked()
    });

    if next_clicked {
        state.error = None;
        let Some(path) = state.data_path.clone() else {
            state.error = Some("Pick a CSV file first.".to_string());
            return;
        };
        match read_full_csv(&path) {
            Ok(df) => {
                let column_names: Vec<String> = df
                    .get_column_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                if !column_names.iter().any(|c| c == "Name") {
                    state.error = Some("Expected a 'Name' column in the input CSV".to_string());
                    return;
                }
                state.has_user_col = column_names.iter().any(|c| c == "User");
                state.column_names = column_names;
                state.df = Some(df);
                state.step = if state.has_user_col {
                    Step::AskUseCodesLookup
                } else {
                    Step::PickSourceOfTruthWorkbook
                };
            }
            Err(e) => state.error = Some(e.to_string()),
        }
    }
}

fn render_ask_use_codes_lookup(ui: &mut egui::Ui, state: &mut State) {
    ui.strong("Printer-codes lookup?");
    let next_clicked = style::card(ui, |ui| {
        let mut use_lookup = state.use_codes_lookup.unwrap_or(true);
        ui.checkbox(
            &mut use_lookup,
            "Use a printer-codes lookup workbook to resolve account IDs to project names before matching?",
        );
        state.use_codes_lookup = Some(use_lookup);

        style::primary_button(ui, "Next").clicked()
    });

    if next_clicked {
        state.error = None;
        let use_lookup = state.use_codes_lookup.unwrap_or(true);
        state.step = if use_lookup {
            Step::PickCodesWorkbook
        } else {
            Step::PickSourceOfTruthWorkbook
        };
    }
}

fn render_pick_codes_workbook(ui: &mut egui::Ui, state: &mut State) {
    ui.strong("Codes workbook");
    let next_clicked = style::card(ui, |ui| {
        widgets::directory_picker(
            ui,
            "cmr_sot_dir",
            "Source of truth folder:",
            &mut state.source_of_truth_dir,
        );
        let files = widgets::list_files_with_extension(&state.source_of_truth_dir, "xlsx");
        if files.is_empty() {
            style::banner(
                ui,
                Status::Error,
                &format!(
                    "No .xlsx files found in {}",
                    state.source_of_truth_dir.display()
                ),
            );
        }
        widgets::file_picker(
            ui,
            "cmr_codes_workbook",
            "Printer CODES lookup workbook (maps account IDs -> project names):",
            &files,
            &mut state.codes_workbook_path,
        );

        style::primary_button(ui, "Next").clicked()
    });

    if next_clicked {
        state.error = None;
        let Some(path) = state.codes_workbook_path.clone() else {
            state.error = Some("Pick a codes workbook first.".to_string());
            return;
        };
        match load_workbook(&path) {
            Ok(workbook) => {
                let sheets = list_sheets(&workbook);
                if sheets.is_empty() {
                    state.error = Some("No sheets found in the codes workbook".to_string());
                    return;
                }
                state.codes_sheet_names = sheets;
                state.codes_workbook = Some(workbook);
                state.step = Step::PickCodesSheet;
            }
            Err(e) => state.error = Some(e),
        }
    }
}

fn render_pick_codes_sheet(ui: &mut egui::Ui, state: &mut State) {
    ui.strong("Codes sheet");
    let next_clicked = style::card(ui, |ui| {
        widgets::string_picker(
            ui,
            "cmr_codes_sheet",
            "Select the codes sheet:",
            &state.codes_sheet_names,
            &mut state.codes_sheet_name,
        );

        style::primary_button(ui, "Next").clicked()
    });

    if next_clicked {
        state.error = None;
        let Some(sheet_name) = state.codes_sheet_name.clone() else {
            state.error = Some("Pick a sheet first.".to_string());
            return;
        };
        let Some(workbook) = state.codes_workbook.as_mut() else {
            state.error = Some("Internal error: codes workbook not loaded".to_string());
            return;
        };
        match parse_codes_sheet(workbook, &sheet_name) {
            Ok(codes_sheet) => {
                if codes_sheet.headers.is_empty() {
                    state.error = Some(format!("Sheet '{sheet_name}' has no header row"));
                    return;
                }
                state.codes_sheet = Some(codes_sheet);
                state.step = Step::PickCodesIdNameColumns;
            }
            Err(e) => state.error = Some(e),
        }
    }
}

fn render_pick_codes_id_name_columns(ui: &mut egui::Ui, state: &mut State) {
    ui.strong("Codes ID / name columns");
    let next_clicked = style::card(ui, |ui| {
        let headers = state
            .codes_sheet
            .as_ref()
            .map(|s| s.headers.clone())
            .unwrap_or_default();
        widgets::string_picker(
            ui,
            "cmr_codes_id_col",
            "Which column is the printer account/User ID?",
            &headers,
            &mut state.codes_id_col,
        );
        widgets::string_picker(
            ui,
            "cmr_codes_name_col",
            "Which column is the Project Name?",
            &headers,
            &mut state.codes_name_col,
        );

        style::primary_button(ui, "Next").clicked()
    });

    if next_clicked {
        state.error = None;
        let (Some(id_col_name), Some(name_col_name)) =
            (state.codes_id_col.clone(), state.codes_name_col.clone())
        else {
            state.error =
                Some("Pick both the account ID column and the project name column.".to_string());
            return;
        };
        let Some(codes_sheet) = state.codes_sheet.as_ref() else {
            state.error = Some("Internal error: codes sheet not loaded".to_string());
            return;
        };
        let (Some(id_col), Some(name_col)) = (
            codes_sheet.headers.iter().position(|h| h == &id_col_name),
            codes_sheet.headers.iter().position(|h| h == &name_col_name),
        ) else {
            state.error =
                Some("Internal error: selected column not found in sheet headers".to_string());
            return;
        };

        let lookup = build_lookup(codes_sheet, id_col, name_col);
        state.codes_loaded_count = Some(lookup.len());
        state.codes_lookup = lookup;

        // Free the workbook handle and parsed sheet; no longer needed.
        state.codes_workbook = None;
        state.codes_sheet = None;

        state.step = Step::PickSourceOfTruthWorkbook;
    }
}

fn render_pick_source_of_truth_workbook(ui: &mut egui::Ui, state: &mut State) {
    ui.strong("CMR tracker workbook");
    let next_clicked = style::card(ui, |ui| {
        if let Some(count) = state.codes_loaded_count {
            ui.label(format!(
                "Loaded {count} account ID -> project name mappings."
            ));
            ui.add_space(4.0);
        }

        widgets::directory_picker(
            ui,
            "cmr_sot_dir2",
            "Source of truth folder:",
            &mut state.source_of_truth_dir,
        );
        let files = widgets::list_files_with_extension(&state.source_of_truth_dir, "xlsx");
        if files.is_empty() {
            style::banner(
                ui,
                Status::Error,
                &format!(
                    "No .xlsx files found in {}",
                    state.source_of_truth_dir.display()
                ),
            );
        }
        widgets::file_picker(
            ui,
            "cmr_sot_workbook",
            "Select the source_of_truth workbook:",
            &files,
            &mut state.workbook_path,
        );

        style::primary_button(ui, "Next").clicked()
    });

    if next_clicked {
        state.error = None;
        let Some(path) = state.workbook_path.clone() else {
            state.error = Some("Pick the source_of_truth workbook first.".to_string());
            return;
        };
        match load_workbook(&path) {
            Ok(workbook) => {
                let sheets = list_sheets(&workbook);
                if sheets.is_empty() {
                    state.error = Some("No sheets found in workbook".to_string());
                    return;
                }
                state.sheet_names = sheets;
                state.workbook = Some(workbook);
                state.step = Step::PickSourceOfTruthSheet;
            }
            Err(e) => state.error = Some(e),
        }
    }
}

fn render_pick_source_of_truth_sheet(ui: &mut egui::Ui, state: &mut State) {
    ui.strong("CMR tracker sheet");
    let next_clicked = style::card(ui, |ui| {
        widgets::string_picker(
            ui,
            "cmr_sot_sheet",
            "Select the month sheet:",
            &state.sheet_names,
            &mut state.sheet_name,
        );

        style::primary_button(ui, "Next").clicked()
    });

    if next_clicked {
        state.error = None;
        let Some(sheet_name) = state.sheet_name.clone() else {
            state.error = Some("Pick a sheet first.".to_string());
            return;
        };
        let Some(workbook) = state.workbook.as_mut() else {
            state.error = Some("Internal error: workbook not loaded".to_string());
            return;
        };
        match parse_sheet(workbook, &sheet_name) {
            Ok(sheet_data) => {
                if sheet_data.blocks.is_empty() {
                    state.error = Some(format!("No CMR blocks found in sheet '{sheet_name}'"));
                    return;
                }
                state.sheet_data = Some(sheet_data);
                state.workbook = None; // No longer needed; free the file handle.
                state.step = Step::PickBlock;
            }
            Err(e) => state.error = Some(e),
        }
    }
}

fn render_pick_block(ui: &mut egui::Ui, state: &mut State) {
    ui.strong("Office / printer block");
    let next_clicked = style::card(ui, |ui| {
        let block_names: Vec<String> = state
            .sheet_data
            .as_ref()
            .map(|sd| sd.blocks.iter().map(|b| b.name.clone()).collect())
            .unwrap_or_default();
        widgets::string_picker(
            ui,
            "cmr_block",
            "Which office/printer block do these readings belong to?",
            &block_names,
            &mut state.block_choice,
        );

        style::primary_button(ui, "Next").clicked()
    });

    if next_clicked {
        state.error = None;
        let Some(choice) = state.block_choice.clone() else {
            state.error = Some("Pick a block first.".to_string());
            return;
        };
        let found = state
            .sheet_data
            .as_ref()
            .and_then(|sd| sd.blocks.iter().find(|b| b.name == choice).cloned());
        let Some(block) = found else {
            state.error = Some("Internal error: selected block not found".to_string());
            return;
        };
        state.block = Some(block);
        state.step = Step::PickPrintCopyColumns;
    }
}

fn render_pick_print_copy_columns(ui: &mut egui::Ui, state: &mut State) {
    ui.strong("Print / copy columns & output");
    let next_clicked = style::card(ui, |ui| {
        widgets::string_picker(
            ui,
            "cmr_print_col",
            "Which column should populate CMR(print)?",
            &state.column_names,
            &mut state.print_col,
        );
        widgets::string_picker(
            ui,
            "cmr_copy_col",
            "Which column should populate CMR(copy)?",
            &state.column_names,
            &mut state.copy_col,
        );

        widgets::directory_picker(
            ui,
            "cmr_output_dir",
            "Output folder (where both CMR reports will be written):",
            &mut state.output_dir,
        );

        style::primary_button(ui, "Next").clicked()
    });

    if next_clicked {
        state.error = None;
        if state.print_col.is_none() || state.copy_col.is_none() {
            state.error =
                Some("Pick both the CMR(print) and CMR(copy) source columns.".to_string());
            return;
        }
        state.current_row = 0;
        state.step = Step::MatchingRow;
    }
}

/// Outcome of resolving a single data row against the source_of_truth
/// project list.
enum RowOutcome {
    /// All rows have been processed.
    Done,
    /// A column lookup that the state machine's own invariants should have
    /// ruled out failed anyway; surfaced instead of panicking.
    Error(String),
    /// No candidates at all — treated as a skip, same as today's logic.
    Skipped(MatchedRow),
    /// Exactly one candidate, and it's an exact code match — resolved
    /// without pausing for user input.
    AutoMatched {
        data_name: String,
        resolved_via: &'static str,
        cmr_print_value: String,
        cmr_copy_value: String,
        candidate: MatchCandidate,
    },
    /// Multiple candidates (or a single non-exact one) — needs user
    /// confirmation.
    Ambiguous(PendingMatch),
}

/// Everything `resolve_row` needs to resolve one row, bundled so the
/// function takes two arguments instead of eight (clippy::too_many_arguments).
#[derive(Clone, Copy)]
struct MatchContext<'a> {
    df: &'a DataFrame,
    sheet_data: &'a SheetData,
    block: &'a Block,
    print_col: &'a str,
    copy_col: &'a str,
    codes_lookup: &'a HashMap<String, String>,
    has_user_col: bool,
}

/// Resolve `row` of `ctx.df` against `ctx.sheet_data.projects`. Pure
/// function: takes everything it needs by reference and returns an
/// outcome, so the caller can apply state mutations afterward without
/// fighting the borrow checker.
fn resolve_row(ctx: &MatchContext<'_>, row: usize) -> RowOutcome {
    let MatchContext {
        df,
        sheet_data,
        block,
        print_col,
        copy_col,
        codes_lookup,
        has_user_col,
    } = *ctx;

    if row >= df.height() {
        return RowOutcome::Done;
    }

    let name_series = match df.column("Name") {
        Ok(s) => s,
        Err(e) => return RowOutcome::Error(e.to_string()),
    };
    let print_series = match df.column(print_col) {
        Ok(s) => s,
        Err(e) => return RowOutcome::Error(e.to_string()),
    };
    let copy_series = match df.column(copy_col) {
        Ok(s) => s,
        Err(e) => return RowOutcome::Error(e.to_string()),
    };

    let data_name = match name_series.get(row) {
        Ok(v) => format!("{v}"),
        Err(e) => return RowOutcome::Error(e.to_string()),
    };
    let cmr_print_value = match print_series.get(row) {
        Ok(v) => format!("{v}"),
        Err(e) => return RowOutcome::Error(e.to_string()),
    };
    let cmr_copy_value = match copy_series.get(row) {
        Ok(v) => format!("{v}"),
        Err(e) => return RowOutcome::Error(e.to_string()),
    };

    // Prefer the printer-codes lookup (keyed by the stable numeric account
    // ID) over fuzzy-matching the raw, sometimes-truncated 'Name' field;
    // fall back to 'Name' when the ID isn't in the map.
    let user_id = if has_user_col {
        df.column("User")
            .ok()
            .and_then(|s| s.get(row).ok())
            .map(|v| format!("{v}"))
            .map(|v| v.trim_matches(|c: char| c == '[' || c == ']').to_string())
    } else {
        None
    };

    let (search_name, resolved_via): (String, &'static str) =
        match user_id.as_deref().and_then(|id| codes_lookup.get(id)) {
            Some(resolved_name) => (resolved_name.clone(), "Codes lookup"),
            None => (data_name.clone(), "Raw name"),
        };

    let candidates = match_candidates(&search_name, &sheet_data.projects);

    if candidates.is_empty() {
        return RowOutcome::Skipped(MatchedRow::unmatched(
            data_name,
            block.name.clone(),
            resolved_via.to_string(),
        ));
    }

    if candidates.len() == 1 && candidates[0].method == MatchMethod::ExactCode {
        return RowOutcome::AutoMatched {
            data_name,
            resolved_via,
            cmr_print_value,
            cmr_copy_value,
            candidate: candidates[0].clone(),
        };
    }

    RowOutcome::Ambiguous(PendingMatch {
        data_name,
        resolved_via,
        cmr_print_value,
        cmr_copy_value,
        candidates,
        choice: 0,
    })
}

/// Apply a confirmed (auto or user-picked) match: record the reading
/// against its source_of_truth project row (warning on overwrite, same as
/// today), and push the audit row.
fn apply_match(
    state: &mut State,
    data_name: String,
    resolved_via: &str,
    cmr_print_value: String,
    cmr_copy_value: String,
    candidate: &MatchCandidate,
) {
    if state
        .readings_by_project
        .insert(
            candidate.row_idx,
            (cmr_print_value.clone(), cmr_copy_value.clone()),
        )
        .is_some()
    {
        state.warnings.push(format!(
            "'{}' already had a reading matched to '{}' — overwriting with this one, check the review file",
            candidate.project_name, data_name
        ));
    }

    let block_name = state
        .block
        .as_ref()
        .expect("state machine guarantees block is set before MatchingRow")
        .name
        .clone();

    state.rows.push(MatchedRow {
        data_name,
        matched_project_name: Some(candidate.project_name.clone()),
        block: block_name,
        cmr_print_value: Some(cmr_print_value),
        cmr_copy_value: Some(cmr_copy_value),
        method: Some(candidate.method.clone()),
        score: Some(candidate.score),
        resolved_via: resolved_via.to_string(),
    });
}

/// Auto-resolve rows (no candidates, or a single exact-code candidate) until
/// either an ambiguous row needs confirmation or every row is processed.
/// Runs synchronously within a single `update()` call — no threads, no
/// `request_repaint`.
fn advance_matching(state: &mut State) {
    loop {
        let (df, sheet_data, block, print_col, copy_col) = match (
            state.df.as_ref(),
            state.sheet_data.as_ref(),
            state.block.as_ref(),
            state.print_col.as_deref(),
            state.copy_col.as_deref(),
        ) {
            (Some(df), Some(sd), Some(b), Some(pc), Some(cc)) => (df, sd, b, pc, cc),
            _ => {
                state.error = Some("Internal error: matching state incomplete".to_string());
                state.step = Step::Done;
                return;
            }
        };

        let ctx = MatchContext {
            df,
            sheet_data,
            block,
            print_col,
            copy_col,
            codes_lookup: &state.codes_lookup,
            has_user_col: state.has_user_col,
        };
        let outcome = resolve_row(&ctx, state.current_row);

        match outcome {
            RowOutcome::Done => {
                state.step = Step::Done;
                return;
            }
            RowOutcome::Error(e) => {
                state.error = Some(e);
                state.step = Step::Done;
                return;
            }
            RowOutcome::Skipped(row) => {
                state.skipped += 1;
                state.rows.push(row);
                state.current_row += 1;
            }
            RowOutcome::AutoMatched {
                data_name,
                resolved_via,
                cmr_print_value,
                cmr_copy_value,
                candidate,
            } => {
                state.auto_matched += 1;
                apply_match(
                    state,
                    data_name,
                    resolved_via,
                    cmr_print_value,
                    cmr_copy_value,
                    &candidate,
                );
                state.current_row += 1;
            }
            RowOutcome::Ambiguous(pending) => {
                state.pending_candidates = Some(pending);
                return;
            }
        }
    }
}

fn render_matching_row(ui: &mut egui::Ui, state: &mut State) {
    let total = state.df.as_ref().map(|d| d.height()).unwrap_or(0);
    let fraction = if total == 0 {
        0.0
    } else {
        state.current_row as f32 / total as f32
    };
    ui.add(egui::ProgressBar::new(fraction).text(format!("{} / {}", state.current_row, total)));
    ui.add_space(8.0);

    let Some(pending) = state.pending_candidates.as_mut() else {
        return;
    };

    ui.label(format!(
        "Confirm match for '{}' [{}]:",
        pending.data_name, pending.resolved_via
    ));

    for (idx, candidate) in pending.candidates.iter().enumerate() {
        ui.radio_value(
            &mut pending.choice,
            idx,
            format!(
                "{} (score {:.2}, {:?})",
                candidate.project_name, candidate.score, candidate.method
            ),
        );
    }
    let skip_index = pending.candidates.len();
    ui.radio_value(&mut pending.choice, skip_index, "No match / skip");

    if ui.button("Confirm").clicked() {
        let pending = state
            .pending_candidates
            .take()
            .expect("checked Some via the let-else above");

        if pending.choice == pending.candidates.len() {
            state.skipped += 1;
            let block_name = state
                .block
                .as_ref()
                .expect("state machine guarantees block is set before MatchingRow")
                .name
                .clone();
            state.rows.push(MatchedRow::unmatched(
                pending.data_name,
                block_name,
                pending.resolved_via.to_string(),
            ));
        } else {
            state.confirmed_matched += 1;
            let candidate = pending.candidates[pending.choice].clone();
            apply_match(
                state,
                pending.data_name,
                pending.resolved_via,
                pending.cmr_print_value,
                pending.cmr_copy_value,
                &candidate,
            );
        }
        state.current_row += 1;
        // Resume auto-resolving subsequent rows within this same click.
        advance_matching(state);
    }
}

/// Build the paste-ready and review reports once, the first time `Step::Done`
/// is reached.
fn finish_matching(state: &mut State) {
    let built = {
        let Some(sheet_data) = state.sheet_data.as_ref() else {
            state.error = Some("Internal error: missing sheet data".to_string());
            return;
        };
        let Some(block) = state.block.as_ref() else {
            state.error = Some("Internal error: missing block".to_string());
            return;
        };
        let Some(data_path) = state.data_path.as_ref() else {
            state.error = Some("Internal error: missing data path".to_string());
            return;
        };

        let project_rows: Vec<ProjectCmr> = sheet_data
            .projects
            .iter()
            .map(|project| {
                let reading = state.readings_by_project.get(&project.row_idx);
                ProjectCmr {
                    row_idx: project.row_idx,
                    project_name: project.project_name.clone(),
                    cmr_print_value: reading.map(|(print, _)| print.clone()),
                    cmr_copy_value: reading.map(|(_, copy)| copy.clone()),
                }
            })
            .collect();

        let stem = data_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output")
            .to_string();

        (project_rows, stem, block.name.clone())
    };
    let (project_rows, stem, block_name) = built;

    match write_paste_ready_report(&project_rows, &state.output_dir, &stem, &block_name) {
        Ok(path) => state.paste_ready_path = Some(path),
        Err(e) => {
            state.error = Some(e);
            return;
        }
    }
    match write_review_report(&state.rows, &state.output_dir, &stem, &block_name) {
        Ok(path) => state.review_path = Some(path),
        Err(e) => state.error = Some(e),
    }
}

fn render_done(ui: &mut egui::Ui, state: &mut State) {
    if state.error.is_some() {
        // Error already rendered at the top of `ui()`.
        return;
    }

    if state.paste_ready_path.is_none() {
        finish_matching(state);
    }

    for warning in &state.warnings {
        style::banner(ui, Status::Warning, warning);
    }

    match (&state.paste_ready_path, &state.review_path) {
        (Some(paste_path), Some(review_path)) => {
            style::card(ui, |ui| {
                style::banner(
                    ui,
                    Status::Success,
                    &format!(
                        "Paste-ready CMR report (workbook order): {}",
                        paste_path.display()
                    ),
                );
                ui.label(format!("Review/audit report: {}", review_path.display()));
                ui.label(format!(
                    "Auto-matched: {}  Confirmed: {}  Needs review: {}",
                    state.auto_matched, state.confirmed_matched, state.skipped
                ));
                if let Some(dir) = paste_path.parent() {
                    widgets::open_output_folder_button(ui, dir);
                }
            });
        }
        _ => {
            if state.error.is_none() {
                style::banner(ui, Status::Error, "Failed to write report files.");
            }
        }
    }
}
