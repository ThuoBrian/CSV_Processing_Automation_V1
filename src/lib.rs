use polars::prelude::*;
use std::fs::File;
use std::path::{Path, PathBuf};

// /// ===============================
// /// Constants for CSV processing
// /// ===============================

// Column names
pub const COL_NAME: &str = "Name";
pub const COL_TOTAL_PRINTS: &str = "Total Prints";
pub const COL_BW_PRINTER: &str = "Black & WhiteTotal(Printer)";
pub const COL_BW_COPIER: &str = "Black & WhiteTotal(Copier/Document Server)";
pub const COL_BW_LARGE: &str = "Black & White(Large size)(Printer)";

/// Helper: return the list of columns we care about
fn selected_columns() -> [&'static str; 5] {
    [
        COL_NAME,
        COL_TOTAL_PRINTS,
        COL_BW_PRINTER,
        COL_BW_COPIER,
        COL_BW_LARGE,
    ]
}

/// Process a CSV file into a cleaned DataFrame
pub fn process_csv_file(input_path: &Path, output_path: &Path) -> Result<DataFrame, PolarsError> {
    // Ensure input file exists
    if !input_path.exists() {
        return Err(PolarsError::ComputeError(
            format!("Input file does not exist: {:?}", input_path).into(),
        ));
    }

    // Open file safely
    let input_file = File::open(input_path).map_err(|error| {
        PolarsError::ComputeError(format!("Failed to open '{:?}': {}", input_path, error).into())
    })?;

    // Read CSV into DataFrame
    let dataframe = CsvReader::new(input_file)
        .has_header(true)
        .finish()
        .map_err(|error| {
            PolarsError::ComputeError(
                format!("Failed to read '{:?}': {}", input_path, error).into(),
            )
        })?;

    // Select only relevant columns
    let selected_dataframe = dataframe.select(selected_columns())?;

    // Clean "Name" column → remove square brackets
    let mut cleaned_dataframe = selected_dataframe
        .lazy()
        .with_column(
            col(COL_NAME)
                .str()
                .replace_all(lit(r#"[\[\]]"#), lit(""), false)
                .alias(COL_NAME),
        )
        .collect()?;

    // Create output file
    let output_file = File::create(output_path).map_err(|error| {
        PolarsError::ComputeError(
            format!("Failed to create '{}': {}", output_path.display(), error).into(),
        )
    })?;

    // Write cleaned DataFrame to CSV
    CsvWriter::new(output_file)
        .has_header(true)
        .finish(&mut cleaned_dataframe)?;

    println!("\n✅ Output file created at: {}", output_path.display());

    Ok(cleaned_dataframe)
}

// /// ===============================
// /// To-do (future improvements)
// /// ===============================
// /// 1. Add logging for debugging purposes
// /// 2. Add support for different CSV formats (e.g., TSV)
// /// 3. Add support for filtering rows based on conditions
// /// 4. Optimize performance for very large CSV files
// /// 5. Support batch processing of multiple files

/// Generate output path from input path
pub fn generate_output_path(input_path: &Path) -> PathBuf {
    let file_name = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let output_dir = input_path.parent().unwrap_or(Path::new("./data"));

    let new_name = format!("{}_Analyzed_Output.csv", file_name);
    output_dir.join(new_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_output_path() {
        let input = Path::new("./data/IPA Busia Printer_usercounter_20260203.csv");
        let output = generate_output_path(input);
        assert!(output.to_string_lossy().contains("Analyzed_Output"));
    }

    #[test]
    fn test_generate_output_path_different_input() {
        let input = Path::new("./data/Test_usercounter_20251203.csv");
        let output = generate_output_path(input);
        assert!(output.to_string_lossy().contains("Analyzed_Output"));
    }
}
