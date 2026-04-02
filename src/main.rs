use clap::Parser;
use csv_processing_automation::{generate_output_path, process_csv_file};
use inquire::{Select, Text, Confirm};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "csv_processing_automation")]
#[command(about = "A tool for processing printer/user-counter CSV data", long_about = None)]
struct Args {
    /// Input CSV file path
    #[arg(short = 'i', long)]
    input: Option<PathBuf>,

    /// Output CSV file path (optional, defaults to auto-generated based on input)
    #[arg(short = 'o', long)]
    output: Option<PathBuf>,

    /// Run in interactive mode
    #[arg(short = 'I', long)]
    interactive: bool,
}

fn process_single_file() -> Result<(), Box<dyn std::error::Error>> {
    // Get input file path
    let default_input = "./data/input.csv".to_string();
    let input_path_str = Text::new("Enter input CSV file path:")
        .with_default(&default_input)
        .prompt()?;
    let input_path = PathBuf::from(input_path_str.trim_matches('"'));

    // Validate input file exists
    if !input_path.exists() {
        eprintln!("Error: Input file does not exist: {:?}", input_path);
        return Err("Input file not found".into());
    }

    // Ask about output path
    let auto_generate = Confirm::new("Generate output path automatically?")
        .with_default(true)
        .prompt()?;

    let output_path = if auto_generate {
        generate_output_path(&input_path)
    } else {
        let default_output = generate_output_path(&input_path)
            .to_string_lossy()
            .to_string();
        let output_path_str = Text::new("Enter output CSV file path:")
            .with_default(&default_output)
            .prompt()?;
        PathBuf::from(output_path_str.trim_matches('"'))
    };

    // Confirm before processing
    let proceed = Confirm::new(&format!(
        "Process '{}' → '{}'?",
        input_path.display(),
        output_path.display()
    ))
    .with_default(true)
    .prompt()?;

    if !proceed {
        println!("Operation cancelled.");
        return Ok(());
    }

    // Process the file
    println!("\nProcessing...");
    match process_csv_file(&input_path, &output_path) {
        Ok(df) => {
            println!("\n✅ Success!");
            println!("   Processed rows: {}", df.height());

            // Ask to preview data
            let preview = Confirm::new("Preview first 5 rows?")
                .with_default(true)
                .prompt()?;

            if preview {
                println!("\n📊 Processed Data (first 5 rows):");
                println!("{}", df.head(Some(5)));
            }

            // Ask to open output directory
            let open_dir = Confirm::new("Open output directory?")
                .with_default(false)
                .prompt()?;

            if open_dir {
                if let Some(dir) = output_path.parent() {
                    open::that(dir).ok();
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to analyze CSV: {:?}", e);
            return Err(Box::new(e));
        }
    }

    Ok(())
}

fn run_interactive_mode() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n╔════════════════════════════════════════════╗");
    println!("║   CSV Processing Automation - Interactive  ║");
    println!("╚════════════════════════════════════════════╝\n");

    loop {
        if let Err(e) = process_single_file() {
            eprintln!("Error: {}", e);
        }

        // Ask if user wants to process another file
        let another = Confirm::new("\nProcess another CSV file?")
            .with_default(true)
            .prompt()?;

        if !another {
            println!("\n👋 Thanks for using CSV Processing Automation!");
            break;
        }

        println!("\n{}", "─".repeat(40));
    }

    Ok(())
}

fn main() {
    let args = Args::parse();

    if args.interactive || (args.input.is_none() && args.output.is_none()) {
        match run_interactive_mode() {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        let input_path = args.input.expect("Input path required");
        let output_path = args
            .output
            .unwrap_or_else(|| generate_output_path(&input_path));

        match process_csv_file(&input_path, &output_path) {
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
}
