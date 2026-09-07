use clap::Parser;
use csv_processing_automation::matcher::{match_candidates, MatchMethod};
use csv_processing_automation::printer_codes::{build_lookup, parse_codes_sheet};
use csv_processing_automation::report::{write_paste_ready_report, write_review_report, MatchedRow, ProjectCmr};
use csv_processing_automation::source_of_truth::{list_sheets, load_workbook, parse_sheet};
use csv_processing_automation::{generate_output_path, process_csv_file, read_full_csv};
use inquire::{Confirm, Select, Text};
use std::collections::HashMap;
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

/// List files in `dir` matching `extension` (e.g. "csv", "xlsx"), sorted by name.
fn list_files_with_extension(dir: &str, extension: &str) -> Vec<PathBuf> {
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

/// Prompt the user to pick a file from `dir` with the given `extension`.
fn select_file(dir: &str, extension: &str, prompt: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let files = list_files_with_extension(dir, extension);
    if files.is_empty() {
        return Err(format!("No .{} files found in {}", extension, dir).into());
    }
    let display: Vec<String> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    let choice = Select::new(prompt, display).prompt()?;
    Ok(files
        .into_iter()
        .find(|p| p.file_name().unwrap().to_string_lossy().to_string() == choice)
        .unwrap())
}

/// Automate matching a printer-usage CSV's readings against the
/// source_of_truth workbook and produce a CMR report CSV.
fn process_cmr_matching() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Pick the data CSV.
    let data_path = select_file("./data", "csv", "Select the printer usage CSV:")?;
    let df = read_full_csv(&data_path)?;

    let column_names: Vec<String> = df
        .get_column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    if !column_names.iter().any(|c| c == "Name") {
        return Err("Expected a 'Name' column in the input CSV".into());
    }
    let has_user_col = column_names.iter().any(|c| c == "User");

    // 1b. Optionally resolve each row's account ID to a clean project name
    // via a printer-codes lookup workbook, before falling back to the raw
    // 'Name' field. This is far more reliable than fuzzy-matching 'Name'.
    let mut codes_lookup: HashMap<String, String> = HashMap::new();
    if has_user_col {
        let use_codes_lookup = Confirm::new(
            "Use a printer-codes lookup workbook to resolve account IDs to project names before matching?",
        )
        .with_default(true)
        .prompt()?;

        if use_codes_lookup {
            let codes_path = select_file(
                "./source_of_truth",
                "xlsx",
                "Select the printer CODES lookup workbook (maps account IDs -> project names):",
            )?;
            let mut codes_workbook =
                load_workbook(&codes_path).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            let codes_sheets = list_sheets(&codes_workbook);
            if codes_sheets.is_empty() {
                return Err("No sheets found in the codes workbook".into());
            }
            let codes_sheet_name = Select::new("Select the codes sheet:", codes_sheets).prompt()?;
            let codes_sheet = parse_codes_sheet(&mut codes_workbook, &codes_sheet_name)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

            if codes_sheet.headers.is_empty() {
                return Err(format!("Sheet '{}' has no header row", codes_sheet_name).into());
            }
            let id_col_name = Select::new(
                "Which column is the printer account/User ID?",
                codes_sheet.headers.clone(),
            )
            .prompt()?;
            let name_col_name = Select::new(
                "Which column is the Project Name?",
                codes_sheet.headers.clone(),
            )
            .prompt()?;
            let id_col = codes_sheet
                .headers
                .iter()
                .position(|h| h == &id_col_name)
                .unwrap();
            let name_col = codes_sheet
                .headers
                .iter()
                .position(|h| h == &name_col_name)
                .unwrap();

            codes_lookup = build_lookup(&codes_sheet, id_col, name_col);
            println!("  Loaded {} account ID -> project name mappings.", codes_lookup.len());
        }
    }

    // 2. Pick the source_of_truth workbook + sheet.
    let workbook_path = select_file(
        "./source_of_truth",
        "xlsx",
        "Select the source_of_truth workbook:",
    )?;
    let mut workbook = load_workbook(&workbook_path).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    let sheets = list_sheets(&workbook);
    if sheets.is_empty() {
        return Err("No sheets found in workbook".into());
    }
    let sheet_name = Select::new("Select the month sheet:", sheets).prompt()?;
    let sheet_data =
        parse_sheet(&mut workbook, &sheet_name).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    if sheet_data.blocks.is_empty() {
        return Err(format!("No CMR blocks found in sheet '{}'", sheet_name).into());
    }

    // 3. Pick which office/printer block this CSV's readings belong to.
    let block_names: Vec<String> = sheet_data.blocks.iter().map(|b| b.name.clone()).collect();
    let block_choice = Select::new(
        "Which office/printer block do these readings belong to?",
        block_names,
    )
    .prompt()?;
    let block = sheet_data
        .blocks
        .iter()
        .find(|b| b.name == block_choice)
        .unwrap()
        .clone();

    // 4. Pick which source columns feed CMR(print) / CMR(copy).
    let print_col = Select::new(
        "Which column should populate CMR(print)?",
        column_names.clone(),
    )
    .prompt()?;
    let copy_col = Select::new(
        "Which column should populate CMR(copy)?",
        column_names.clone(),
    )
    .prompt()?;

    // 5. Match each data row against the source_of_truth project list.
    let name_series = df.column("Name")?;
    let user_series = if has_user_col { Some(df.column("User")?) } else { None };
    let print_series = df.column(&print_col)?;
    let copy_series = df.column(&copy_col)?;

    let mut rows: Vec<MatchedRow> = Vec::new();
    // row_idx (of the matched source_of_truth project) -> (CMR print, CMR copy).
    let mut readings_by_project: HashMap<usize, (String, String)> = HashMap::new();
    let mut auto_matched = 0usize;
    let mut confirmed_matched = 0usize;
    let mut skipped = 0usize;

    for i in 0..df.height() {
        let data_name = format!("{}", name_series.get(i)?);
        let cmr_print_value = format!("{}", print_series.get(i)?);
        let cmr_copy_value = format!("{}", copy_series.get(i)?);

        // Prefer the printer-codes lookup (keyed by the stable numeric
        // account ID) over fuzzy-matching the raw, sometimes-truncated
        // 'Name' field; fall back to 'Name' when the ID isn't in the map.
        let user_id = user_series
            .map(|s| format!("{}", s.get(i).unwrap_or_default()))
            .map(|v| v.trim_matches(|c: char| c == '[' || c == ']').to_string());
        let (search_name, resolved_via): (String, &str) = match user_id.as_deref().and_then(|id| codes_lookup.get(id))
        {
            Some(resolved_name) => (resolved_name.clone(), "Codes lookup"),
            None => (data_name.clone(), "Raw name"),
        };

        let candidates = match_candidates(&search_name, &sheet_data.projects);

        let chosen = if candidates.len() == 1 && candidates[0].method == MatchMethod::ExactCode {
            auto_matched += 1;
            println!(
                "  ✅ Auto-matched '{}' [{}] → '{}'",
                data_name, resolved_via, candidates[0].project_name
            );
            Some(candidates[0].clone())
        } else if candidates.is_empty() {
            println!(
                "  ⚠️  No candidates found for '{}' [{}, searched as '{}']",
                data_name, resolved_via, search_name
            );
            None
        } else {
            let mut options: Vec<String> = candidates
                .iter()
                .map(|c| format!("{} (score {:.2}, {:?})", c.project_name, c.score, c.method))
                .collect();
            options.push("No match / skip".to_string());
            let choice = Select::new(
                &format!("Confirm match for '{}' [{}]:", data_name, resolved_via),
                options.clone(),
            )
            .prompt()?;
            if choice == "No match / skip" {
                None
            } else {
                let idx = options.iter().position(|o| o == &choice).unwrap();
                confirmed_matched += 1;
                Some(candidates[idx].clone())
            }
        };

        match chosen {
            Some(candidate) => {
                if readings_by_project
                    .insert(candidate.row_idx, (cmr_print_value.clone(), cmr_copy_value.clone()))
                    .is_some()
                {
                    println!(
                        "  ⚠️  '{}' already had a reading matched to '{}' — overwriting with this one, check the review file",
                        candidate.project_name, data_name
                    );
                }
                rows.push(MatchedRow {
                    data_name,
                    matched_project_name: Some(candidate.project_name),
                    block: block.name.clone(),
                    cmr_print_value: Some(cmr_print_value),
                    cmr_copy_value: Some(cmr_copy_value),
                    method: Some(candidate.method),
                    score: Some(candidate.score),
                    resolved_via: resolved_via.to_string(),
                });
            }
            None => {
                skipped += 1;
                rows.push(MatchedRow::unmatched(data_name, block.name.clone(), resolved_via.to_string()));
            }
        }
    }

    // 6. Build the paste-ready report: one row per source_of_truth project,
    // in the workbook's own order, with the exact Project Name text.
    let project_rows: Vec<ProjectCmr> = sheet_data
        .projects
        .iter()
        .map(|project| {
            let reading = readings_by_project.get(&project.row_idx);
            ProjectCmr {
                row_idx: project.row_idx,
                project_name: project.project_name.clone(),
                cmr_print_value: reading.map(|(print, _)| print.clone()),
                cmr_copy_value: reading.map(|(_, copy)| copy.clone()),
            }
        })
        .collect();

    // 7. Write both reports.
    let stem = data_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let paste_ready_path = write_paste_ready_report(&project_rows, stem, &block.name)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    let review_path = write_review_report(&rows, stem, &block.name)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    println!(
        "\n✅ Paste-ready CMR report (workbook order): {}",
        paste_ready_path.display()
    );
    println!("   Review/audit report: {}", review_path.display());
    println!(
        "   Auto-matched: {}  Confirmed: {}  Needs review: {}",
        auto_matched, confirmed_matched, skipped
    );

    let open_dir = Confirm::new("Open output directory?")
        .with_default(false)
        .prompt()?;
    if open_dir {
        if let Some(dir) = paste_ready_path.parent() {
            open::that(dir).ok();
        }
    }

    Ok(())
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
        let mode = Select::new(
            "What would you like to do?",
            vec![
                "Clean a single printer CSV",
                "Match printer readings to source_of_truth (CMR report)",
            ],
        )
        .prompt()?;

        let result = if mode == "Match printer readings to source_of_truth (CMR report)" {
            process_cmr_matching()
        } else {
            process_single_file()
        };

        if let Err(e) = result {
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
