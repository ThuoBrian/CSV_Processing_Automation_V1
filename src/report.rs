use crate::matcher::MatchMethod;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// One resolved (or explicitly skipped) row, keyed off the raw printer-CSV
/// reading. Written to the "review" file for audit purposes.
pub struct MatchedRow {
    pub data_name: String,
    pub matched_project_name: Option<String>,
    pub block: String,
    pub cmr_print_value: Option<String>,
    pub cmr_copy_value: Option<String>,
    pub method: Option<MatchMethod>,
    pub score: Option<f64>,
    /// How the search name fed into the matcher was resolved:
    /// "Codes lookup" (via the printer-codes workbook) or "Raw name"
    /// (the printer CSV's own Name field).
    pub resolved_via: String,
}

impl MatchedRow {
    pub fn unmatched(data_name: String, block: String, resolved_via: String) -> Self {
        Self {
            data_name,
            matched_project_name: None,
            block,
            cmr_print_value: None,
            cmr_copy_value: None,
            method: None,
            score: None,
            resolved_via,
        }
    }
}

/// One row of the paste-ready report: a source_of_truth project, in its
/// original sheet order, with the CMR reading found for it (if any).
pub struct ProjectCmr {
    pub row_idx: usize,
    pub project_name: String,
    pub cmr_print_value: Option<String>,
    pub cmr_copy_value: Option<String>,
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn output_path(input_stem: &str, block: &str, suffix: &str) -> Result<PathBuf, String> {
    let output_dir = Path::new("./output");
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output dir: {}", e))?;
    let safe_block = block.replace(|c: char| !c.is_alphanumeric(), "_");
    Ok(output_dir.join(format!("{}_CMR_Report_{}{}.csv", input_stem, safe_block, suffix)))
}

/// Write the paste-ready CMR report: one row per source_of_truth project,
/// in the same order as the workbook sheet, with the exact `Project Name`
/// text so the CMR(print)/CMR(copy) columns can be copy-pasted straight
/// into the workbook without re-sorting or re-matching by hand.
pub fn write_paste_ready_report(
    rows: &[ProjectCmr],
    input_stem: &str,
    block: &str,
) -> Result<PathBuf, String> {
    let path = output_path(input_stem, block, "")?;
    let mut sorted: Vec<&ProjectCmr> = rows.iter().collect();
    sorted.sort_by_key(|r| r.row_idx);

    let mut file =
        File::create(&path).map_err(|e| format!("Failed to create '{}': {}", path.display(), e))?;

    writeln!(file, "Project Name,CMR(print),CMR(copy)").map_err(|e| e.to_string())?;
    for row in sorted {
        writeln!(
            file,
            "{},{},{}",
            csv_escape(&row.project_name),
            csv_escape(row.cmr_print_value.as_deref().unwrap_or("")),
            csv_escape(row.cmr_copy_value.as_deref().unwrap_or("")),
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(path)
}

/// Write the audit/review report: one row per printer-CSV reading, showing
/// what it matched to (or that it didn't), for spot-checking before you
/// trust the paste-ready report.
pub fn write_review_report(
    rows: &[MatchedRow],
    input_stem: &str,
    block: &str,
) -> Result<PathBuf, String> {
    let path = output_path(input_stem, block, "_Review")?;
    let mut file =
        File::create(&path).map_err(|e| format!("Failed to create '{}': {}", path.display(), e))?;

    writeln!(
        file,
        "Data Name,Matched Project Name,Block,CMR(print),CMR(copy),Match Method,Confidence,Resolved Via"
    )
    .map_err(|e| e.to_string())?;

    for row in rows {
        let matched_project_name = row
            .matched_project_name
            .clone()
            .unwrap_or_else(|| "NO MATCH - manual review".to_string());
        let method = match &row.method {
            Some(MatchMethod::ExactCode) => "Exact code",
            Some(MatchMethod::Fuzzy) => "Fuzzy",
            None => "",
        };
        let confidence = row.score.map(|s| format!("{:.2}", s)).unwrap_or_default();

        writeln!(
            file,
            "{},{},{},{},{},{},{},{}",
            csv_escape(&row.data_name),
            csv_escape(&matched_project_name),
            csv_escape(&row.block),
            csv_escape(row.cmr_print_value.as_deref().unwrap_or("")),
            csv_escape(row.cmr_copy_value.as_deref().unwrap_or("")),
            csv_escape(method),
            csv_escape(&confidence),
            csv_escape(&row.resolved_via),
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(path)
}
