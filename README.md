# CSV Processing Automation

A simple Rust tool for processing printer/user-counter CSV data.

## What it does

- Reads printer usage CSV files
- Cleans the "Name" column (removes brackets)
- Selects relevant columns only
- Outputs a cleaned CSV file
- **CMR matching**: matches `./data/*.csv` printer readings against the
  `./source_of_truth/*.xlsx` project tracker (by fuzzy/exact project-code
  matching) and writes a CMR(print)/CMR(copy) report CSV to `./output/` —
  see [CMR matching workflow](#cmr-matching-workflow) below.

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

## CMR matching workflow

Run interactively (`cargo run -- -I`) and choose **"Match printer readings to
source_of_truth (CMR report)"**. You'll be prompted to:

1. Pick a printer usage CSV from `./data/`.
1b. (Optional, on by default) Resolve each row's numeric account ID to a
   clean project name via a printer-codes lookup workbook — pick the
   workbook/sheet from `./source_of_truth/` and which columns are the
   account ID and the Project Name. This is far more reliable than
   matching the CSV's raw `Name` field directly, since account IDs are
   stable while `Name` text can be truncated/reordered. Rows whose account
   ID isn't in the lookup fall back to matching on the raw `Name` field.
2. Pick the source_of_truth CMR tracking workbook from `./source_of_truth/`
   and a month sheet.
3. Pick which office/printer block (e.g. `NAIROBI - PROGRAMS`) the CSV's
   readings belong to.
4. Pick which CSV column feeds `CMR(print)` and which feeds `CMR(copy)`.
5. Confirm each project match — exact project-code matches are auto-accepted
   and logged; anything ambiguous or fuzzy prompts you to pick from the top
   candidates (or skip).

Two report CSVs are written to `./output/`:

- `<input>_CMR_Report_<block>.csv` — **paste-ready**: one row per
  source_of_truth project, in the exact same order as the workbook sheet,
  with columns `Project Name, CMR(print), CMR(copy)`. The `Project Name`
  text matches source_of_truth exactly, so you can copy the CMR(print)/
  CMR(copy) columns straight into the workbook without re-sorting or
  re-matching by hand. Projects with no reading this period are left blank.
- `<input>_CMR_Report_<block>_Review.csv` — audit trail: one row per
  printer-CSV reading, showing what it matched to (or didn't — flagged
  `NO MATCH - manual review`), the match method, confidence score, and
  whether it was resolved via the codes lookup or the raw `Name` fallback.

**This does not edit the source_of_truth workbook** — these are review/paste
reports, not an in-place update.

## Project Structure

- `src/main.rs` - CLI entry point
- `src/lib.rs` - Core processing logic
- `src/source_of_truth.rs` - Reads project names + CMR column layout from the xlsx tracker
- `src/printer_codes.rs` - Reads the account-ID -> project-name lookup workbook
- `src/matcher.rs` - Exact/fuzzy project-code matching
- `src/report.rs` - Writes the CMR match report CSVs
- `data/` - Input CSV files
- `source_of_truth/` - The project/CMR tracking workbook and the printer-codes lookup workbook
