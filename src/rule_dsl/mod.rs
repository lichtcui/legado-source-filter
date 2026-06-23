pub mod combinator;
pub mod html_extractor;
pub mod json_extractor;
pub mod parser;

use crate::types::SearchRule;

/// Parse a rule string and extract values from content (HTML or JSON).
pub fn extract_results(
    content: &str,
    content_type: &str,
    rule: &SearchRule,
) -> Vec<Vec<String>> {
    let mut results = Vec::new();

    // Apply bookList to get items
    if let Some(ref bl) = rule.bookList {
        let tokens = parser::tokenize(bl);
        let items = if content_type.contains("json") {
            if let Ok(json) = serde_json::from_str(content) {
                json_extractor::extract(&json, &tokens)
            } else {
                Vec::new()
            }
        } else {
            html_extractor::extract(content, &tokens)
        };

        for _item in &items {
            // For each item, extract name and author
            let name = apply_extract(content, content_type, &rule.name, 0);
            let author = apply_extract(content, content_type, &rule.author, 0);
            let book_url = apply_extract(content, content_type, &rule.bookUrl, 0);
            let cover_url = apply_extract(content, content_type, &rule.coverUrl, 0);

            results.push(vec![
                name.first().cloned().unwrap_or_default(),
                author.first().cloned().unwrap_or_default(),
                book_url.first().cloned().unwrap_or_default(),
                cover_url.first().cloned().unwrap_or_default(),
            ]);
        }
    }

    results
}

/// Check if search results contain valid data (non-empty name or author).
pub fn has_valid_results(results: &[Vec<String>]) -> bool {
    if results.is_empty() {
        return false;
    }
    for row in results {
        if row.len() >= 2 {
            let name = row[0].trim();
            let author = row[1].trim();
            if !name.is_empty() || !author.is_empty() {
                return true;
            }
        }
    }
    false
}

fn apply_extract(content: &str, content_type: &str, rule: &Option<String>, _index: usize) -> Vec<String> {
    match rule {
        Some(r) if !r.is_empty() => {
            let tokens = parser::tokenize(r);
            let results = if content_type.contains("json") {
                if let Ok(json) = serde_json::from_str(content) {
                    json_extractor::extract(&json, &tokens)
                } else {
                    Vec::new()
                }
            } else {
                html_extractor::extract(content, &tokens)
            };
            // Apply combinator tokens (||, ##, !0) to the extracted results
            combinator::apply_combinators(results, &tokens)
        }
        _ => Vec::new(),
    }
}
