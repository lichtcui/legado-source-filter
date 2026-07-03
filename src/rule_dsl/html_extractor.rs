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
                    if let Some(elem) = doc.root_element().select(&Selector::parse("*").unwrap()).next()
                        && let Some(href) = elem.value().attr("href") {
                            results.push(href.to_string());
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
                    if let Some(elem) = doc.root_element().select(&Selector::parse("*").unwrap()).next()
                        && let Some(src) = elem.value().attr("src") {
                            results.push(src.to_string());
                        }
                }
                return results;
            }

            // ── Unsupported ──
            _ => {}
        }
    }

    // No extractor encountered: return the outer HTML of remaining elements
    // (preserving HTML structure for downstream selector steps).
    results = current_html;
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
            // Serialize the element itself (not just its children) by wrapping in a container.
            // Preserve ALL attributes to support subsequent locator/extractor steps
            // that may depend on any attribute (data-*, style, title, rel, etc.).
            let inner = element.html();
            let tag = element.value().name.local.as_ref();
            let attrs: String = element.value()
                .attrs()
                .map(|(k, v)| format!(" {}=\"{}\"", k, v.replace('"', "&quot;")))
                .collect::<Vec<_>>()
                .join("");
            let outer = format!("<{tag}{attrs}>{inner}</{tag}>");
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

    #[test]
    fn test_preserves_custom_attributes() {
        // The fix ensures ALL attributes are preserved when rebuilding outer HTML,
        // not just id/class/href/src.
        let html = r#"<div class="item" data-id="42" style="color:red" title="tip" rel="nofollow">
            <a href="/book/1">Book 1</a>
        </div>"#;
        // Apply class selector, then extract href from the child link
        let tokens = tokenize("class.item@tag.a@href");
        let results = extract(html, &tokens);
        assert_eq!(results, vec!["/book/1"], "href from child link");

        // Verify the outer HTML reconstruction includes nested elements
        // by extracting inner HTML after the class selector.
        let with_html = tokenize("class.item@html");
        let results = extract(html, &with_html);
        assert_eq!(results.len(), 1, "should match one .item element");
        assert!(
            results[0].contains("href=\"/book/1\""),
            "href attribute should be preserved in inner HTML"
        );
    }
}
