use regex::Regex;

use super::parser::RuleToken;

/// Apply a chain of rule tokens against extracted strings.
///
/// Handles `||` fallback, `##` regex replacement, and `!0` index exclusion.
pub fn apply_combinators(values: Vec<String>, tokens: &[RuleToken]) -> Vec<String> {
    let mut results = values;
    let mut remaining = tokens;

    while !remaining.is_empty() {
        let token = &remaining[0];
        remaining = &remaining[1..];

        match token {
            RuleToken::Fallback => {
                // If we already have results, stop — || means "use first non-empty"
                if !results.is_empty() && !results.iter().all(|s| s.trim().is_empty()) {
                    return results;
                }
                // Otherwise clear and continue with next segment
                results.clear();
            }
            RuleToken::RegexReplace { pattern, replacement } => {
                if let Ok(re) = Regex::new(pattern) {
                    results = results
                        .into_iter()
                        .map(|s| re.replace_all(&s, replacement).to_string())
                        .collect();
                }
            }
            RuleToken::Index(n) => {
                // !0 means "exclude index 0", .0 means "only index 0"
                // We handle Index as "take index n" in the HTML extractor.
                // Here we handle it as "remove index n" for exclusion patterns.
                if *n < results.len() {
                    results.remove(*n);
                }
            }
            _ => {}
        }
    }

    results
}

/// Merge multiple rule result sets using `||` semantics:
/// Return the first non-empty set, or empty if all empty.
pub fn merge_fallback(results: &[Vec<String>]) -> Vec<String> {
    for r in results {
        if !r.is_empty() && !r.iter().all(|s| s.trim().is_empty()) {
            return r.clone();
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_dsl::parser::tokenize;

    #[test]
    fn test_regex_replace() {
        let tokens = tokenize("$.name##(\\d+)##NUM:$1");
        // tokens: [JsonPath, RegexReplace]
        assert_eq!(tokens.len(), 2);
    }

    #[test]
    fn test_fallback_semantics() {
        let first = vec!["".to_string()];
        let second = vec!["result".to_string()];
        let merged = merge_fallback(&[first, second]);
        assert_eq!(merged, vec!["result"]);
    }
}
