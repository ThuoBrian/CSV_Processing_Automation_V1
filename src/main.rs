use csv_processing_automation::*;
use std::path::Path;

fn main() {
    match process_csv_file(Path::new(INPUT_CSV_FILE)) {
        Ok(df) => {
            println!("Processed DataFrame:\n{}", df.head(Some(5)));
        }
        Err(e) => {
            eprintln!("Failed to analyze CSV: {:?}", e);
        }
    }
}
