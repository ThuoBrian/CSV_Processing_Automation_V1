use clap::Parser;
use csv_processing_automation::{generate_output_path, process_csv_file};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "csv_processing_automation")]
#[command(about = "A tool for processing printer/user-counter CSV data", long_about = None)]
struct Args {
    /// Input CSV file path
    #[arg(short, long)]
    input: PathBuf,

    /// Output CSV file path (optional, defaults to auto-generated based on input)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    let output_path = args.output.unwrap_or_else(|| generate_output_path(&args.input));

    match process_csv_file(&args.input, &output_path) {
        Ok(df) => {
            println!("\nProcessed rows: {}", df.height());
            println!("\nProcessed DataFrame:\n{}", df.head(Some(5)));
        }
        Err(e) => {
            eprintln!("Failed to analyze CSV: {:?}", e);
            std::process::exit(1);
        }
    }
}
