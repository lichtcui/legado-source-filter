pub mod runner;

/// Classify a JS search URL by complexity.
pub fn classify_js(search_url: &str) -> JsLevel {
    if search_url.starts_with("@js:") {
        let body = &search_url[4..];
        if body.contains("java.startBrowserAwait") {
            return JsLevel::L3Unrecoverable;
        }
        if body.contains("java.ajax") || body.contains("java.post") || body.contains("cookie.") {
            return JsLevel::L3Polyfillable;
        }
        if body.contains("function") || body.contains("try {") || body.contains("JSON.stringify")
            || body.matches('\n').count() > 3
        {
            return JsLevel::L2;
        }
        JsLevel::L1
    } else if search_url.contains("<js>") {
        if search_url.contains("java.startBrowserAwait") {
            return JsLevel::L3Unrecoverable;
        }
        if search_url.contains("java.ajax") || search_url.contains("java.post") {
            return JsLevel::L3Polyfillable;
        }
        JsLevel::L2
    } else if search_url.contains("{{") {
        JsLevel::Template
    } else {
        JsLevel::PlainUrl
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsLevel {
    /// Simple concatenation — Rust-side regex handles it
    L1,
    /// Function/branch logic — needs node
    L2,
    /// Uses Java APIs — node + polyfill
    L3Polyfillable,
    /// Uses startBrowserAwait — cannot test
    L3Unrecoverable,
    /// Template `{{key}}` — no JS needed
    Template,
    /// Plain URL — no JS needed
    PlainUrl,
}
