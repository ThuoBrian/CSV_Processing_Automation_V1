use calamine::{open_workbook, Data, Reader, Xlsx};
use std::path::Path;

/// The row (1-indexed in the spreadsheet, 0-indexed here) that carries the
/// office/printer block labels, e.g. "NAIROBI - PROGRAMS".
const BLOCK_LABEL_ROW: usize = 0;
/// The row that carries the actual column headers, e.g. "CMR(print)".
const HEADER_ROW: usize = 4;
/// Data rows start right after the header row.
const DATA_START_ROW: usize = 5;

/// A single office/printer block (e.g. "NAIROBI - PROGRAMS") found in the
/// sheet's header row. Which columns feed CMR(print)/CMR(copy) is picked
/// interactively by the caller, so this only carries the block's name.
#[derive(Debug, Clone)]
pub struct Block {
    pub name: String,
}

/// One project row from the source_of_truth sheet.
#[derive(Debug, Clone)]
pub struct ProjectRow {
    /// 0-indexed row offset within the sheet (for reference/debugging).
    pub row_idx: usize,
    pub project_name: String,
}

/// Parsed contents of a single month sheet that we care about.
#[derive(Debug, Clone)]
pub struct SheetData {
    pub blocks: Vec<Block>,
    pub projects: Vec<ProjectRow>,
}

/// Open an xlsx workbook for reading.
pub fn load_workbook(path: &Path) -> Result<Xlsx<std::io::BufReader<std::fs::File>>, String> {
    open_workbook(path).map_err(|e| format!("Failed to open workbook '{}': {}", path.display(), e))
}

/// List the sheet names in a workbook, in their on-disk order.
pub fn list_sheets(workbook: &Xlsx<std::io::BufReader<std::fs::File>>) -> Vec<String> {
    workbook.sheet_names().to_vec()
}

/// Parse a single month sheet into its office/printer blocks and project rows.
pub fn parse_sheet(
    workbook: &mut Xlsx<std::io::BufReader<std::fs::File>>,
    sheet_name: &str,
) -> Result<SheetData, String> {
    let range = workbook
        .worksheet_range(sheet_name)
        .map_err(|e| format!("Failed to read sheet '{}': {}", sheet_name, e))?;

    let header_row: Vec<String> = range
        .rows()
        .nth(HEADER_ROW)
        .ok_or_else(|| format!("Sheet '{}' has no header row at index {}", sheet_name, HEADER_ROW))?
        .iter()
        .map(cell_to_string)
        .collect();

    let block_label_row: Vec<String> = range
        .rows()
        .nth(BLOCK_LABEL_ROW)
        .map(|row| row.iter().map(cell_to_string).collect())
        .unwrap_or_default();

    let blocks = find_blocks(&block_label_row, &header_row);

    let projects: Vec<ProjectRow> = range
        .rows()
        .enumerate()
        .skip(DATA_START_ROW)
        .filter_map(|(idx, row)| {
            let name = row.first().map(cell_to_string).unwrap_or_default();
            let trimmed = name.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("total") {
                None
            } else {
                Some(ProjectRow {
                    row_idx: idx,
                    project_name: trimmed.to_string(),
                })
            }
        })
        .collect();

    Ok(SheetData { blocks, projects })
}

fn cell_to_string(cell: &Data) -> String {
    cell.to_string()
}

/// Walk the header row looking for "CMR(print)"/"CMR(copy)" pairs, and
/// attribute each pair to the nearest preceding block label (carried
/// forward across the merged cells in `block_label_row`).
fn find_blocks(block_label_row: &[String], header_row: &[String]) -> Vec<Block> {
    // Carry the block label forward across merged/blank cells.
    let mut labels: Vec<String> = Vec::with_capacity(header_row.len());
    let mut current = String::new();
    for i in 0..header_row.len() {
        if let Some(label) = block_label_row.get(i) {
            if !label.trim().is_empty() {
                current = label.trim().to_string();
            }
        }
        labels.push(current.clone());
    }

    let mut blocks: Vec<Block> = Vec::new();
    let mut saw_print_col = false;
    for (i, header) in header_row.iter().enumerate() {
        let h = header.trim().to_ascii_lowercase();
        if h.contains("cmr") && h.contains("print") {
            saw_print_col = true;
        } else if h.contains("cmr") && h.contains("copy") {
            if saw_print_col {
                saw_print_col = false;
                let name = labels.get(i).cloned().unwrap_or_default();
                if !name.is_empty() {
                    blocks.push(Block { name });
                }
            }
        }
    }
    blocks
}
