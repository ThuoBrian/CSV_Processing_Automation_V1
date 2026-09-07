use calamine::{Data, Reader, Xlsx};
use std::collections::HashMap;
use std::io::BufReader;

/// A generic (headers + data rows) view of one sheet in a printer-codes
/// lookup workbook. Unlike the CMR workbook, these sheets vary widely in
/// layout across years, but the sheets that matter for current matching
/// have a single clean header row (row 1), which is all we assume here.
pub struct CodesSheet {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

fn cell_to_string(cell: &Data) -> String {
    cell.to_string()
}

/// Parse a codes-workbook sheet: row 1 is the header, everything after is data.
pub fn parse_codes_sheet(
    workbook: &mut Xlsx<BufReader<std::fs::File>>,
    sheet_name: &str,
) -> Result<CodesSheet, String> {
    let range = workbook
        .worksheet_range(sheet_name)
        .map_err(|e| format!("Failed to read sheet '{}': {}", sheet_name, e))?;

    let mut rows_iter = range.rows();
    let headers: Vec<String> = rows_iter
        .next()
        .map(|row| row.iter().map(cell_to_string).collect())
        .unwrap_or_default();

    let rows: Vec<Vec<String>> = rows_iter
        .map(|row| row.iter().map(cell_to_string).collect())
        .collect();

    Ok(CodesSheet { headers, rows })
}

/// Normalize an account-ID cell for use as a lookup key: trims whitespace
/// and a trailing ".0" left over from calamine rendering a whole-number
/// float cell (e.g. "9905.0" -> "9905").
fn normalize_id(raw: &str) -> String {
    let trimmed = raw.trim();
    trimmed.strip_suffix(".0").unwrap_or(trimmed).to_string()
}

/// Build an account-ID -> project-name lookup from a parsed codes sheet.
/// Rows with an empty id or name are skipped.
pub fn build_lookup(sheet: &CodesSheet, id_col: usize, name_col: usize) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for row in &sheet.rows {
        let id = row.get(id_col).map(|s| normalize_id(s)).unwrap_or_default();
        let name = row.get(name_col).map(|s| s.trim().to_string()).unwrap_or_default();
        if !id.is_empty() && !name.is_empty() {
            map.insert(id, name);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> CodesSheet {
        CodesSheet {
            headers: vec![
                "Project Name - Project Grant Code".to_string(),
                "ShortCode".to_string(),
                "User".to_string(),
            ],
            rows: vec![
                vec![
                    "Universal Basic Income_13126AA_Kenya1_GDR0001".to_string(),
                    "13126AA_Kenya1_GDR0001".to_string(),
                    "9905.0".to_string(),
                ],
                vec![
                    "Memory and Animal Health Project (MAH)_21087AA_Kenya1_WSU-22-10001".to_string(),
                    "Kenya1_WSU-22-10001".to_string(),
                    "9860.0".to_string(),
                ],
                // Missing id -> should be skipped.
                vec!["No Account Project".to_string(), "".to_string(), "".to_string()],
            ],
        }
    }

    #[test]
    fn normalize_id_strips_trailing_dot_zero() {
        assert_eq!(normalize_id("9905.0"), "9905");
        assert_eq!(normalize_id(" 9905 "), "9905");
    }

    #[test]
    fn build_lookup_maps_id_to_project_name() {
        let sheet = fixture();
        let map = build_lookup(&sheet, 2, 0);
        assert_eq!(
            map.get("9905").map(|s| s.as_str()),
            Some("Universal Basic Income_13126AA_Kenya1_GDR0001")
        );
        assert_eq!(
            map.get("9860").map(|s| s.as_str()),
            Some("Memory and Animal Health Project (MAH)_21087AA_Kenya1_WSU-22-10001")
        );
        assert_eq!(map.len(), 2);
    }
}
