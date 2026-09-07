use crate::source_of_truth::ProjectRow;

/// Below this Jaro-Winkler score, a fuzzy candidate isn't worth showing.
const FUZZY_FLOOR: f64 = 0.75;
/// How many fuzzy candidates to keep for interactive review.
const MAX_CANDIDATES: usize = 5;
/// A code-shaped token must carry at least this many digits to count as a
/// grant/project code rather than a generic word like "Kenya1" or "Office2".
const MIN_CODE_DIGITS: usize = 2;

#[derive(Debug, Clone, PartialEq)]
pub enum MatchMethod {
    ExactCode,
    Fuzzy,
}

#[derive(Debug, Clone)]
pub struct MatchCandidate {
    pub project_name: String,
    pub row_idx: usize,
    pub score: f64,
    pub method: MatchMethod,
}

fn strip_noise(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '[' && *c != ']' && *c != '\u{00ad}')
        .collect()
}

/// Normalize a raw name for comparison: strip brackets/soft hyphens,
/// uppercase, collapse separators to single spaces, trim.
pub fn normalize(s: &str) -> String {
    let upper = strip_noise(s).to_uppercase();
    let collapsed: String = upper
        .chars()
        .map(|c| if c == '_' || c == '-' || c.is_whitespace() { ' ' } else { c })
        .collect();
    collapsed.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract project/grant-code-shaped tokens from a name, e.g. "18001AA",
/// "BMG-19-10001", "AMI0001". Tokens are split on underscores/whitespace
/// (so internal hyphens like in "BMG-19-10001" survive as one token), then
/// kept only if they carry enough digits to be code-shaped rather than a
/// generic word like "Kenya1" or "Office2".
pub fn extract_codes(s: &str) -> Vec<String> {
    strip_noise(s)
        .split(|c: char| c == '_' || c.is_whitespace())
        .map(|tok| {
            tok.trim_matches(|c: char| !c.is_alphanumeric() && c != '-')
                .to_uppercase()
        })
        .filter(|tok| tok.chars().filter(|c| c.is_ascii_digit()).count() >= MIN_CODE_DIGITS)
        .collect()
}

/// Rank source_of_truth project rows against one data-file `Name` value.
///
/// 1. Exact code-token intersection wins outright (score 1.0, method ExactCode).
/// 2. Otherwise fall back to Jaro-Winkler similarity on normalized full
///    strings, keeping the top candidates above `FUZZY_FLOOR`.
pub fn match_candidates(data_name: &str, projects: &[ProjectRow]) -> Vec<MatchCandidate> {
    let data_codes = extract_codes(data_name);

    let mut exact: Vec<MatchCandidate> = Vec::new();
    if !data_codes.is_empty() {
        for project in projects {
            let project_codes = extract_codes(&project.project_name);
            if data_codes.iter().any(|c| project_codes.contains(c)) {
                exact.push(MatchCandidate {
                    project_name: project.project_name.clone(),
                    row_idx: project.row_idx,
                    score: 1.0,
                    method: MatchMethod::ExactCode,
                });
            }
        }
    }

    if !exact.is_empty() {
        return exact;
    }

    let normalized_data = normalize(data_name);
    let mut fuzzy: Vec<MatchCandidate> = projects
        .iter()
        .map(|project| {
            let score = strsim::jaro_winkler(&normalized_data, &normalize(&project.project_name));
            MatchCandidate {
                project_name: project.project_name.clone(),
                row_idx: project.row_idx,
                score,
                method: MatchMethod::Fuzzy,
            }
        })
        .filter(|c| c.score >= FUZZY_FLOOR)
        .collect();

    fuzzy.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    fuzzy.truncate(MAX_CANDIDATES);
    fuzzy
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str) -> ProjectRow {
        ProjectRow {
            row_idx: 0,
            project_name: name.to_string(),
        }
    }

    #[test]
    fn normalize_strips_brackets_and_collapses_separators() {
        assert_eq!(normalize("[12062AA_Kenya1_BMG00]"), "12062AA KENYA1 BMG00");
    }

    #[test]
    fn extract_codes_finds_grant_style_tokens() {
        let codes = extract_codes("Evaluating a Management Intervention_18001AA_Kenya1_AMI0001");
        assert!(codes.contains(&"18001AA".to_string()));
        assert!(codes.contains(&"AMI0001".to_string()));
        // "Kenya1" is a generic location tag (only 1 digit), not a grant code.
        assert!(!codes.contains(&"KENYA1".to_string()));
    }

    #[test]
    fn extract_codes_keeps_hyphenated_grant_codes_intact() {
        let codes = extract_codes("Impact of Basic Income_13126BB_Kenya1_BMG-18-10001");
        assert!(codes.contains(&"BMG-18-10001".to_string()));
    }

    #[test]
    fn exact_code_match_wins_over_fuzzy() {
        let projects = vec![
            project("Evaluating a Management Intervention_18001AA_Kenya1_AMI0001"),
            project("Evaluating a Management Intervention in East Africa_CEP-19-10002_18001AB"),
        ];
        let candidates = match_candidates("[12062AA_Kenya1_BMG00]", &[
            project("Some Other Project_12062AA_Kenya1_BMG00"),
        ]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].method, MatchMethod::ExactCode);
        assert_eq!(candidates[0].score, 1.0);

        // 18001AA vs 18001AB must NOT be confused by exact-code matching.
        let candidates = match_candidates("[18001AA_Kenya1_AMI0001]", &projects);
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].project_name.contains("AMI0001"));
    }

    #[test]
    fn no_candidates_below_fuzzy_floor() {
        let projects = vec![project("Completely Unrelated Project Title_99999ZZ")];
        let candidates = match_candidates("[12062AA_Kenya1_BMG00]", &projects);
        assert!(candidates.is_empty());
    }
}
