use scraper::{Html, Selector};

use super::parser::RuleToken;

/// Extract text or attribute values from an HTML string using rule tokens.
///
/// Each selector step re-parses the matched elements' HTML to avoid
/// complex lifetime chains with ElementRef.
pub fn extract(html: &str, tokens: &[RuleToken]) -> Vec<String> {
    let mut current_html = vec![html.to_string()];
    let mut current_tokens = tokens;
    let mut results = Vec::new();

    while !current_tokens.is_empty() {
        let token = &current_tokens[0];
        current_tokens = &current_tokens[1..];

        match token {
            // ── Locators: narrow by selector ──
            RuleToken::CssClass(class) => {
                let sel_str = format!(".{}", class);
                current_html = apply_selector_all(&current_html, &sel_str);
            }
            RuleToken::CssId(id) => {
                let sel_str = format!("#{}", id);
                current_html = apply_selector_all(&current_html, &sel_str);
            }
            RuleToken::TagName(tag) => {
                current_html = apply_selector_all(&current_html, tag);
            }
            RuleToken::CssOverride(css) => {
                current_html = apply_selector_all(&current_html, css);
            }
            RuleToken::TagShort(tag) => {
                current_html = apply_selector_all(&current_html, tag);
            }

            // ── Index ──
            RuleToken::Index(n) => {
                current_html = current_html.into_iter().skip(*n).take(1).collect();
            }

            // ── Extractors: final step ──
            RuleToken::Text => {
                for html_snippet in &current_html {
                    let doc = Html::parse_fragment(html_snippet);
                    results.push(doc.root_element().text().collect::<String>());
                }
                return results;
            }
            RuleToken::TextNodes => {
                for html_snippet in &current_html {
                    let doc = Html::parse_fragment(html_snippet);
                    let text: String = doc.root_element()
                        .children()
                        .filter_map(|child| {
                            match child.value() {
                                scraper::node::Node::Text(t) => Some(t.text.to_string()),
                                _ => None,
                            }
                        })
                        .collect();
                    results.push(text);
                }
                return results;
            }
            RuleToken::Href => {
                for html_snippet in &current_html {
                    let doc = Html::parse_fragment(html_snippet);
                    if let Some(elem) = doc.root_element().select(&Selector::parse("*").unwrap()).next() {
                        if let Some(href) = elem.value().attr("href") {
                            results.push(href.to_string());
                        }
                    }
                }
                return results;
            }
            RuleToken::Html => {
                for html_snippet in &current_html {
                    let doc = Html::parse_fragment(html_snippet);
                    results.push(doc.root_element().inner_html());
                }
                return results;
            }
            RuleToken::Src => {
                for html_snippet in &current_html {
                    let doc = Html::parse_fragment(html_snippet);
                    if let Some(elem) = doc.root_element().select(&Selector::parse("*").unwrap()).next() {
                        if let Some(src) = elem.value().attr("src") {
                            results.push(src.to_string());
                        }
                    }
                }
                return results;
            }

            // ── Unsupported ──
            _ => {}
        }
    }

    // No extractor encountered: return text from remaining elements
    for html_snippet in &current_html {
        let doc = Html::parse_fragment(html_snippet);
        results.push(doc.root_element().text().collect::<String>());
    }
    results
}

/// Apply a CSS selector to each HTML snippet, returning outer HTML of matched elements.
/// This preserves the element context for subsequent locators and extractors.
fn apply_selector_all(snippets: &[String], selector_str: &str) -> Vec<String> {
    let mut results = Vec::new();
    let selector = match Selector::parse(selector_str) {
        Ok(s) => s,
        Err(_) => return results,
    };

    for snippet in snippets {
        let doc = Html::parse_fragment(snippet);
        for element in doc.root_element().select(&selector) {
            // Serialize the element itself (not just its children) by wrapping in a container
            let inner = element.html();
            let tag = element.value().name.local.as_ref();
            let id = element.value().id().map(|i| format!(" id=\"{}\"", i)).unwrap_or_default();
            let class_attr = element.value().classes().collect::<Vec<_>>();
            let class_str = if class_attr.is_empty() { String::new() } else { format!(" class=\"{}\"", class_attr.join(" ")) };

            // Build attributes string (simplified — just href and src for now)
            let href = element.value().attr("href").map(|h| format!(" href=\"{}\"", h)).unwrap_or_default();
            let src = element.value().attr("src").map(|s| format!(" src=\"{}\"", s)).unwrap_or_default();

            let outer = format!("<{tag}{id}{class_str}{href}{src}>{inner}</{tag}>");
            results.push(outer);
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_dsl::parser::tokenize;

    #[test]
    fn test_simple_text() {
        let html = r#"<div class="item"><a href="/book/1">Book 1</a></div>"#;
        let tokens = tokenize("class.item@tag.a@text");
        let results = extract(html, &tokens);
        assert_eq!(results, vec!["Book 1"]);
    }

    #[test]
    fn test_href() {
        let html = r#"<div class="item"><a href="/book/1">Title</a></div>"#;
        let tokens = tokenize("class.item@tag.a@href");
        let results = extract(html, &tokens);
        assert_eq!(results, vec!["/book/1"]);
    }

    #[test]
    fn test_index_modifier() {
        let html = r#"<ul><li>First</li><li>Second</li></ul>"#;
        let tokens = tokenize("tag.li.0@text");
        let results = extract(html, &tokens);
        assert_eq!(results, vec!["First"]);
    }
}
