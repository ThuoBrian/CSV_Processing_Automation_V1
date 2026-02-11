# CSV Processing Automation

A simple Rust tool for processing printer/user-counter CSV data.

## What it does

- Reads printer usage CSV files
- Cleans the "Name" column (removes brackets)
- Selects relevant columns only
- Outputs a cleaned CSV file

## How to use

### Prerequisites
- Rust and Cargo installed

### Build
```bash
cargo build --release
```

### Run
```bash
# Simple (output auto-generated)
cargo run --release -- --input data/IPA\ Busia\ Printer_usercounter_20260203.csv

# With custom output
cargo run --release -- --input data/input.csv --output data/output.csv
```

### Help
```bash
cargo run --release -- --help
```

## Project Structure

- `src/main.rs` - CLI entry point
- `src/lib.rs` - Core processing logic
- `data/` - Input/output CSV files
