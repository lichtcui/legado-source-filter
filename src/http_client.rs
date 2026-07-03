use std::time::Duration;

use encoding_rs::Encoding;
use reqwest::{Client, ClientBuilder};

use crate::search_url::RequestSpec;

pub struct HttpClient {
    client: Client,
}

#[derive(Debug)]
pub struct FetchResult {
    pub body_text: String,
    pub body_bytes: Vec<u8>,
    pub content_type: String,
    pub status_code: u16,
    pub elapsed_ms: u64,
    pub charset_detected: Option<String>,
    pub charset_uncertain: bool,
}

impl HttpClient {
    pub fn new(timeout_secs: u64) -> anyhow::Result<Self> {
        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(timeout_secs))
            .connect_timeout(Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Linux; Android 13; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36")
            .gzip(true)
            .https_only(false)
            .build()?;

        Ok(Self { client })
    }

    pub async fn fetch(&self, spec: &RequestSpec) -> anyhow::Result<FetchResult> {
        let start = std::time::Instant::now();

        let response = match spec.method.as_str() {
            "POST" => {
                let mut req = self.client.post(&spec.url);
                if let Some(ref body) = spec.body {
                    // Replace templates in body too
                    req = req.body(body.clone());
                }
                for (k, v) in &spec.headers {
                    req = req.header(k, v);
                }
                req.send().await?
            }
            _ => {
                let mut req = self.client.get(&spec.url);
                for (k, v) in &spec.headers {
                    req = req.header(k, v);
                }
                req.send().await?
            }
        };

        let status_code = response.status().as_u16();

        // Server errors (5xx) are transient — return error so tester retries
        if status_code >= 500 {
            anyhow::bail!("HTTP {} for {}", status_code, spec.url);
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let elapsed_ms = start.elapsed().as_millis() as u64;
        let body_bytes = response.bytes().await?.to_vec();

        // Decode bytes to string using charset detection
        let (body_text, charset_detected, charset_uncertain) = decode_body(&body_bytes, &content_type, &spec.charset);

        Ok(FetchResult {
            body_text,
            body_bytes,
            content_type,
            status_code,
            elapsed_ms,
            charset_detected,
            charset_uncertain,
        })
    }
}

/// Parse the Python-dict style header string into key-value pairs.
/// Format: `{'User-Agent': 'Mozilla/5.0'}` or `{"key": "value"}`
pub fn parse_headers(header_str: &str) -> Vec<(String, String)> {
    use regex::Regex;
    let mut headers = Vec::new();
    if header_str.is_empty() {
        return headers;
    }

    // Match `'key': 'value'` or `"key": "value"`
    let re = Regex::new(r#"['"]([^'"]+)['"]\s*:\s*['"]([^'"]*)['"]"#).unwrap();
    for cap in re.captures_iter(header_str) {
        headers.push((cap[1].to_string(), cap[2].to_string()));
    }
    headers
}

fn decode_body(bytes: &[u8], content_type: &str, url_charset: &Option<String>) -> (String, Option<String>, bool) {
    // Priority 1: Content-Type header charset
    if let Some(charset_str) = extract_charset_from_header(content_type)
        && let Some(encoding) = Encoding::for_label(charset_str.as_bytes()) {
            let (text, _, _) = encoding.decode(bytes);
            return (text.into_owned(), Some(charset_str), false);
        }

    // Priority 2: Charset hint from searchUrl config
    if let Some(charset) = url_charset
        && let Some(encoding) = Encoding::for_label(charset.as_bytes()) {
            let (text, _, had_errors) = encoding.decode(bytes);
            if !had_errors {
                return (text.into_owned(), Some(charset.clone()), false);
            }
        }

    // Priority 3: chardetng detection
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(bytes, bytes.len() < 1024);
    let encoding = detector.guess(None, true);
    let (text, _, had_errors) = encoding.decode(bytes);
    let name = encoding.name().to_lowercase();
    let uncertain = had_errors || bytes.len() < 64;

    (text.into_owned(), Some(name), uncertain)
}

fn extract_charset_from_header(content_type: &str) -> Option<String> {
    let lower = content_type.to_lowercase();
    if let Some(pos) = lower.find("charset=") {
        let rest = &lower[pos + 8..];
        let charset = rest.split([';', ' ', ',']).next().unwrap_or("");
        if !charset.is_empty() {
            return Some(charset.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_headers_simple() {
        let h = parse_headers("{'User-Agent': 'Mozilla/5.0'}");
        assert_eq!(h, vec![("User-Agent".to_string(), "Mozilla/5.0".to_string())]);
    }

    #[test]
    fn test_parse_headers_multi() {
        let h = parse_headers("{'key1': 'val1', 'key2': 'val2'}");
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].0, "key1");
        assert_eq!(h[1].0, "key2");
    }

    #[test]
    fn test_extract_charset_from_header() {
        assert_eq!(
            extract_charset_from_header("text/html; charset=gbk"),
            Some("gbk".into())
        );
        assert_eq!(
            extract_charset_from_header("text/html"),
            None
        );
    }
}
