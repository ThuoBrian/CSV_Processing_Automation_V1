use polars::prelude::*;
use std::fs::File;
use std::path::Path;

// /// ===============================
// /// Constants for CSV processing
// /// ===============================

// Column names
pub const COL_NAME: &str = "Name";
pub const COL_TOTAL_PRINTS: &str = "Total Prints";
pub const COL_BW_PRINTER: &str = "Black & WhiteTotal(Printer)";
pub const COL_BW_COPIER: &str = "Black & WhiteTotal(Copier/Document Server)";
pub const COL_BW_LARGE: &str = "Black & White(Large size)(Printer)";

// File paths
pub const INPUT_CSV_FILE: &str = "./data/IPA Busia Printer_usercounter_20251203.csv";
pub const OUTPUT_CSV_FILE: &str = "./data/Busia_Printer_Analyzed_Output.csv";

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
pub fn process_csv_file(input_path: &Path) -> Result<DataFrame, PolarsError> {
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
    let cleaned_dataframe = selected_dataframe
        .lazy()
        .with_column(
            col(COL_NAME)
                .str()
                .replace_all(lit(r#"[\[\]]"#), lit(""), false)
                .alias(COL_NAME),
        )
        .collect()?;

    // Create output file
    let mut output_file = File::create(OUTPUT_CSV_FILE).map_err(|error| {
        PolarsError::ComputeError(
            format!("Failed to create '{}': {}", OUTPUT_CSV_FILE, error).into(),
        )
    })?;

    // Write cleaned DataFrame to CSV
    let mut cleaned_dataframe = cleaned_dataframe;
    CsvWriter::new(&mut output_file)
        .has_header(true)
        .finish(&mut cleaned_dataframe)?;

    println!("\n✅ Output file created at: {}", OUTPUT_CSV_FILE);

    Ok(cleaned_dataframe)
}

/* To-do:

1. Add error handling for CSV writing
2. Add logging for debugging purposes
3. Add unit tests for the process_csv_file function
4. Add support for different CSV formats (e.g., TSV)
5. Add support for filtering rows based on conditions
6. Use Clap for command-line argument parsing
7. Optimize performance for large CSV files

*/
