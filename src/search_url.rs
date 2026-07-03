use regex::Regex;

use crate::types::BookSource;

/// Specification for an HTTP request.
#[derive(Debug, Clone)]
pub struct RequestSpec {
    pub url: String,
    pub method: String,
    pub body: Option<String>,
    pub headers: Vec<(String, String)>,
    pub charset: Option<String>,
}

/// Build an HTTP request spec from a book source and keyword.
/// Returns None if the search URL type is not supported by this phase.
pub fn build_request(source: &BookSource, keyword: &str) -> Option<RequestSpec> {
    let search_url = source.searchUrl.as_deref()?;
    let base_url = &source.bookSourceUrl;

    // Determine the effective search URL (resolve @js: L1 patterns in Rust)
    let effective_url = resolve_js_l1(search_url, base_url, keyword)
        .unwrap_or_else(|| resolve_template(search_url, keyword));

    // Check for POST format: path,{'method':'POST','body':'...'}
    if let Some(spec) = try_parse_post(&effective_url, base_url) {
        return Some(spec);
    }

    // Default: GET request
    let full_url = resolve_url(&effective_url, base_url);
    Some(RequestSpec {
        url: full_url,
        method: "GET".to_string(),
        body: None,
        headers: Vec::new(),
        charset: None,
    })
}

/// Resolve `@js:` L1 patterns with Rust-side string processing.
fn resolve_js_l1(search_url: &str, base_url: &str, keyword: &str) -> Option<String> {
    if !search_url.starts_with("@js:") {
        return None;
    }

    let js_body = &search_url[4..];

    // Match: url = baseUrl + "/path/" + key  (possibly with encodeURI wrapping)
    let key_enc = urlencoding(keyword);
    let re = Regex::new(
        r#"(?s)url\s*=\s*baseUrl\s*\+\s*"([^"]*)"\s*\+\s*(?:encodeURI\s*\(\s*)?key(?:\))?"#
    ).ok()?;
    if let Some(caps) = re.captures(js_body) {
        let path = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let path_resolved = resolve_template(path, &key_enc);
        return Some(format!("{}{}", base_url.trim_end_matches('/'), path_resolved));
    }

    None
}

/// Replace `{{key}}`, `{{page}}` templates in a string.
fn resolve_template(s: &str, keyword: &str) -> String {
    s.replace("{{key}}", keyword)
        .replace("{{page}}", "1")
        .replace("{{KEY}}", keyword)
}

/// Resolve a possibly-relative URL against a base URL.
fn resolve_url(url: &str, base: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        return url.to_string();
    }
    let base = base.trim_end_matches('/');
    let url = url.trim_start_matches('/');
    format!("{}/{}", base, url)
}

/// Try to parse a search URL as a POST request in Legado format: `path,{dict}`.
fn try_parse_post(search_url: &str, base_url: &str) -> Option<RequestSpec> {
    // Check if there's a dict-style method declaration
    let has_post = search_url.contains("'method':'POST'")
        || search_url.contains("\"method\":\"POST\"")
        || search_url.contains("'method':'post'")
        || search_url.contains("\"method\":\"post\"");

    if !has_post {
        return None;
    }

    // Extract body content
    let body = extract_post_body(search_url)?;
    let charset = extract_charset(search_url);

    // The URL is whatever comes before the first `,{`
    let url_part = if let Some(pos) = search_url.find(",{") {
        &search_url[..pos]
    } else if let Some(pos) = search_url.find(", {\"method\"") {
        &search_url[..pos]
    } else if let Some(pos) = search_url.find(", {'method'") {
        &search_url[..pos]
    } else {
        search_url
    };

    let full_url = resolve_url(url_part.trim(), base_url);

    Some(RequestSpec {
        url: full_url,
        method: "POST".to_string(),
        body: Some(body),
        headers: Vec::new(),
        charset,
    })
}

fn extract_post_body(s: &str) -> Option<String> {
    // Try single-quote: 'body':'...'
    let re_single = Regex::new(r"'body'\s*:\s*'([^']*)'").ok()?;
    if let Some(caps) = re_single.captures(s) {
        return Some(caps[1].to_string());
    }
    // Try double-quote: "body":"..."
    let re_double = Regex::new(r#""body"\s*:\s*"([^"]*)""#).ok()?;
    if let Some(caps) = re_double.captures(s) {
        return Some(caps[1].to_string());
    }
    None
}

fn extract_charset(s: &str) -> Option<String> {
    // Try 'charset':'xxx'
    let re_single = Regex::new(r"'charset'\s*:\s*'([^']*)'").ok()?;
    if let Some(caps) = re_single.captures(s) {
        return Some(caps[1].to_string().to_lowercase());
    }
    // Try "charset":"xxx"
    let re_double = Regex::new(r#""charset"\s*:\s*"([^"]*)""#).ok()?;
    if let Some(caps) = re_double.captures(s) {
        return Some(caps[1].to_string().to_lowercase());
    }
    None
}

fn urlencoding(s: &str) -> String {
    // Simple URL encoding similar to JavaScript's encodeURI (not encodeURIComponent)
    // For Chinese characters, Python's urllib.parse.quote is closer.
    // We'll use percent-encoding for non-ASCII chars
    let mut result = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            0x20 => result.push('+'),
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_replacement() {
        let result = resolve_template("/search?keyword={{key}}&page={{page}}", "测试");
        assert_eq!(result, "/search?keyword=测试&page=1");
    }

    #[test]
    fn test_url_resolve_relative() {
        assert_eq!(
            resolve_url("/search", "https://example.com"),
            "https://example.com/search"
        );
    }

    #[test]
    fn test_url_resolve_absolute() {
        assert_eq!(
            resolve_url("https://other.com/search", "https://example.com"),
            "https://other.com/search"
        );
    }

    #[test]
    fn test_post_body_extraction() {
        let s = "/modules/article/search.php,{'charset':'gbk','body':'searchkey={{key}}','method':'POST'}";
        let body = extract_post_body(s);
        assert_eq!(body, Some("searchkey={{key}}".into()));

        let charset = extract_charset(s);
        assert_eq!(charset, Some("gbk".into()));
    }
}
