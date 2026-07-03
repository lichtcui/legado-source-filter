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

        for item in &items {
            // For each item, extract name and author within the item's context
            // (not the full document). This ensures item-level rules produce
            // correct per-item results.
            let name = apply_extract(item, content_type, &rule.name, 0);
            let author = apply_extract(item, content_type, &rule.author, 0);
            let book_url = apply_extract(item, content_type, &rule.bookUrl, 0);
            let cover_url = apply_extract(item, content_type, &rule.coverUrl, 0);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SearchRule;

    #[test]
    fn test_extract_results_html_multi_item() {
        // Two items in a list — each should extract its own name and author.
        // The fix (item-level context) ensures results are per-item, not full-document.
        let html = r#"<ul>
            <li class="book"><span class="title">Book1</span><span class="author">Author1</span></li>
            <li class="book"><span class="title">Book2</span><span class="author">Author2</span></li>
        </ul>"#;

        // bookList items should preserve HTML structure for item-level extraction
        let rule = SearchRule {
            bookList: Some("class.book".into()),
            name: Some("class.title@text".into()),
            author: Some("class.author@text".into()),
            ..Default::default()
        };

        let results = extract_results(html, "html", &rule);
        assert_eq!(results.len(), 2, "should extract 2 items");
        assert_eq!(results[0][0], "Book1", "first item name");
        assert_eq!(results[0][1], "Author1", "first item author");
        assert_eq!(results[1][0], "Book2", "second item name");
        assert_eq!(results[1][1], "Author2", "second item author");
    }

    #[test]
    fn test_extract_results_json_multi_item() {
        let json = r#"{"books": [
            {"title": "Book1", "author": "Author1"},
            {"title": "Book2", "author": "Author2"}
        ]}"#;
        let rule = SearchRule {
            bookList: Some("$.books[*]".into()),
            name: Some("$.title".into()),
            author: Some("$.author".into()),
            ..Default::default()
        };

        let results = extract_results(json, "json", &rule);
        assert_eq!(results.len(), 2, "should extract 2 items");
        assert_eq!(results[0][0], "Book1", "first item name");
        assert_eq!(results[1][0], "Book2", "second item name");
        assert_eq!(results[0][1], "Author1", "first item author");
    }

    #[test]
    fn test_has_valid_results() {
        assert!(!has_valid_results(&[]), "empty list should be invalid");
        assert!(!has_valid_results(&[vec!["".into(), "".into()]]), "empty fields should be invalid");
        assert!(has_valid_results(&[vec!["Book".into(), "".into()]]), "name only should be valid");
        assert!(has_valid_results(&[vec!["".into(), "Author".into()]]), "author only should be valid");
    }
}
