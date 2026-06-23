/// Legado rule DSL lexer & parser.
///
/// Tokenizes a rule string like `class.item@tag.li.0@text` into a chain
/// of locator, index, and extractor tokens.

#[derive(Debug, Clone, PartialEq)]
pub enum RuleToken {
    // ── Locators ──
    /// `class.name`
    CssClass(String),
    /// `id.name`
    CssId(String),
    /// `tag.name`
    TagName(String),
    /// `@css:selector`
    CssOverride(String),
    /// `$.path` or `@JSon:path`
    JsonPath(String),
    /// `data.path`
    DataPath(Vec<String>),
    /// `<js>code</js>`
    JsBlock(String),

    // ── Modifiers ──
    /// `.n` → take index n
    Index(usize),

    // ── Extractors ──
    /// `@text`
    Text,
    /// `@textNodes` — direct text nodes only
    TextNodes,
    /// `@href`
    Href,
    /// `@html` — inner HTML
    Html,
    /// `@src`
    Src,

    // ── Combinators ──
    /// `||` — fallback separator
    Fallback,
    /// `##pattern##replacement`
    RegexReplace { pattern: String, replacement: String },

    // ── Special ──
    /// Bare element name like `@a`, `@li` — shorthand tag selector
    TagShort(String),
    /// `@js:code`
    JsTransform(String),
    /// Unknown — forward compat
    Unknown(String),
}

/// Split a rule string at `||` boundaries, then tokenize each segment.
pub fn tokenize(rule: &str) -> Vec<RuleToken> {
    let mut tokens = Vec::new();
    if rule.is_empty() {
        return tokens;
    }

    // First, handle <js> blocks at the very start
    let remaining = if rule.trim_start().starts_with("<js>") {
        let s = rule.trim_start();
        if let Some(end) = s.find("</js>") {
            let code = &s[4..end];
            tokens.push(RuleToken::JsBlock(code.to_string()));
            let after = s[end + 5..].trim();
            if after.is_empty() {
                return tokens;
            }
            after
        } else {
            tokens.push(RuleToken::JsBlock(s[4..].to_string()));
            return tokens;
        }
    } else {
        rule
    };

    // Now handle || and ## and @-based tokens
    let segments = split_fallback(remaining);
    for (i, seg) in segments.iter().enumerate() {
        if i > 0 {
            tokens.push(RuleToken::Fallback);
        }
        if let Some(re) = parse_regex_replace(seg) {
            tokens.extend(re);
        } else {
            tokens.extend(tokenize_at_chain(seg));
        }
    }

    tokens
}

fn split_fallback(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let bytes = s.as_bytes();
    for i in 0..bytes.len().saturating_sub(1) {
        if bytes[i] == b'|' && bytes[i + 1] == b'|' {
            let part = s[start..i].trim();
            if !part.is_empty() {
                parts.push(part);
            }
            start = i + 2;
        }
    }
    if start < s.len() {
        let part = s[start..].trim();
        if !part.is_empty() {
            parts.push(part);
        }
    }
    parts
}

/// Try to parse a segment as `rule##pattern##replacement`.
fn parse_regex_replace(s: &str) -> Option<Vec<RuleToken>> {
    let _bytes = s.as_bytes();
    // Find first ## that separates rule from regex
    let first = s.find("##")?;
    // After first ##, find second ## that separates regex from replacement
    let after_first = &s[first + 2..];
    let second = after_first.find("##")?;
    let rule_part = &s[..first];
    let pattern = &after_first[..second];
    let replacement = &after_first[second + 2..];

    let mut tokens = tokenize_at_chain(rule_part);
    tokens.push(RuleToken::RegexReplace {
        pattern: pattern.to_string(),
        replacement: replacement.to_string(),
    });
    Some(tokens)
}

/// Tokenize a segment by `@` separators into a chain.
fn tokenize_at_chain(s: &str) -> Vec<RuleToken> {
    let mut tokens = Vec::new();
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return tokens;
    }

    let parts: Vec<&str> = trimmed.split('@').collect();
    for (i, part) in parts.iter().enumerate() {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if i == 0 {
            // First part: locator (may include index)
            let (loc, idx) = split_index(p);
            tokens.push(parse_locator(&loc));
            if let Some(n) = idx {
                tokens.push(RuleToken::Index(n));
            }
        } else {
            // @-prefixed part
            let (annot, idx) = split_index(p);
            tokens.push(parse_annotation(&annot));
            if let Some(n) = idx {
                tokens.push(RuleToken::Index(n));
            }
        }
    }

    tokens
}

/// Split a name like "item.0" into ("item", Some(0)).
fn split_index(s: &str) -> (&str, Option<usize>) {
    let bytes = s.as_bytes();
    let len = bytes.len();

    // Find the last '.' or '!' followed by digits at the end
    if len < 3 {
        return (s, None);
    }

    // Scan backwards from the end
    let mut digit_end = len;
    while digit_end > 0 && bytes[digit_end - 1].is_ascii_digit() {
        digit_end -= 1;
    }

    if digit_end > 0 && digit_end < len && (bytes[digit_end - 1] == b'.' || bytes[digit_end - 1] == b'!') {
        if let Ok(num) = s[digit_end..].parse::<usize>() {
            return (&s[..digit_end - 1], Some(num));
        }
    }
    (s, None)
}

fn parse_locator(s: &str) -> RuleToken {
    if s.starts_with("class.") {
        RuleToken::CssClass(s[6..].to_string())
    } else if s.starts_with("id.") {
        RuleToken::CssId(s[3..].to_string())
    } else if s.starts_with("tag.") {
        RuleToken::TagName(s[4..].to_string())
    } else if s.starts_with("$.") || s.starts_with("$[") {
        RuleToken::JsonPath(s.to_string())
    } else if s.starts_with("data.") {
        let path: Vec<String> = s[5..].split('.').map(|x| x.to_string()).collect();
        RuleToken::DataPath(path)
    } else if s.starts_with("//") {
        RuleToken::Unknown(s.to_string())
    } else {
        RuleToken::Unknown(s.to_string())
    }
}

fn parse_annotation(s: &str) -> RuleToken {
    match s {
        "text" | "textNodes" => {
            if s == "textNodes" {
                RuleToken::TextNodes
            } else {
                RuleToken::Text
            }
        }
        "href" => RuleToken::Href,
        "html" => RuleToken::Html,
        "src" => RuleToken::Src,
        _ => {
            if s.starts_with("css:") {
                RuleToken::CssOverride(s[4..].to_string())
            } else if s.starts_with("JSon:") {
                RuleToken::JsonPath(s[5..].to_string())
            } else if s.starts_with("tag.") {
                RuleToken::TagName(s[4..].to_string())
            } else if s.starts_with("js:") {
                RuleToken::JsTransform(s[3..].to_string())
            } else {
                // Bare element/tag like @a, @li, @div
                RuleToken::TagShort(s.to_string())
            }
        }
    }
}

// ── Public convenience ──

pub fn parse_rule(rule: &str) -> Vec<RuleToken> {
    tokenize(rule)
}

pub fn extract_index(s: &str) -> Option<(String, usize)> {
    let (base, idx) = split_index(s);
    idx.map(|n| (base.to_string(), n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_css() {
        let t = tokenize("class.item@tag.li@text");
        assert_eq!(t, vec![
            RuleToken::CssClass("item".into()),
            RuleToken::TagName("li".into()),
            RuleToken::Text,
        ]);
    }

    #[test]
    fn test_index_modifier() {
        let t = tokenize("class.item.0@tag.a@href");
        assert_eq!(t[0], RuleToken::CssClass("item".into()));
        assert_eq!(t[1], RuleToken::Index(0));
        assert_eq!(t[2], RuleToken::TagName("a".into()));
        assert_eq!(t[3], RuleToken::Href);
    }

    #[test]
    fn test_jsonpath() {
        let t = tokenize("$.data[*].name");
        assert_eq!(t, vec![RuleToken::JsonPath("$.data[*].name".into())]);
    }

    #[test]
    fn test_fallback() {
        let t = tokenize("class.a@li@text||class.b@li@text");
        assert_eq!(t.len(), 7, "3 tokens per segment + 1 Fallback = 7");
        assert!(t.contains(&RuleToken::Fallback));
    }

    #[test]
    fn test_regex_replace() {
        let t = tokenize("$.name##foo##bar");
        assert_eq!(t.len(), 2);
        assert_eq!(t[0], RuleToken::JsonPath("$.name".into()));
        assert!(matches!(t[1], RuleToken::RegexReplace { .. }));
    }

    #[test]
    fn test_split_index() {
        assert_eq!(split_index("item.0"), ("item", Some(0)));
        assert_eq!(split_index("item!0"), ("item", Some(0)));
        assert_eq!(split_index("item.12"), ("item", Some(12)));
        assert_eq!(split_index("item"), ("item", None));
    }

    #[test]
    fn test_annotation() {
        assert_eq!(parse_annotation("text"), RuleToken::Text);
        assert_eq!(parse_annotation("textNodes"), RuleToken::TextNodes);
        assert_eq!(parse_annotation("href"), RuleToken::Href);
        assert_eq!(parse_annotation("a"), RuleToken::TagShort("a".into()));
    }
}
