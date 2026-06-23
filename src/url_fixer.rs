/// Auto-fix malformed `bookSourceUrl` values.
///
/// Returns `Some(fixed_url)` if fixable, `None` if irreparably broken.

pub fn fix_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();

    // Already valid
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }

    // [图片]http://... → strip prefix
    if let Some(pos) = trimmed.find("http") {
        if pos > 0 && trimmed[..pos].contains("图片") {
            return Some(trimmed[pos..].to_string());
        }
    }

    // http:www.xxx.com → add slash
    if trimmed.starts_with("http:") && !trimmed.starts_with("http://") {
        return Some(trimmed.replacen("http:", "http://", 1));
    }

    // www.xxx.com or m.xxx.com → prepend https://
    if trimmed.starts_with("www.") || trimmed.starts_with("m.") {
        return Some(format!("https://{}", trimmed));
    }

    // something like "novel.html5.qq.com" — domain-like, no protocol
    if let Some(dot_pos) = trimmed.find('.') {
        let before_dot = &trimmed[..dot_pos];
        // must start with an alphanumeric char (not whitespace, not emoji)
        if before_dot.chars().next().map_or(false, |c| c.is_alphanumeric())
            && !trimmed.contains(char::is_whitespace)
            && !trimmed.contains("：")
        {
            return Some(format!("https://{}", trimmed));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_already_valid() {
        assert_eq!(fix_url("https://example.com"), Some("https://example.com".into()));
        assert_eq!(fix_url("http://example.com"), Some("http://example.com".into()));
    }

    #[test]
    fn test_strip_image_prefix() {
        assert_eq!(
            fix_url("[图片]http://www.sfacg.com"),
            Some("http://www.sfacg.com".into())
        );
    }

    #[test]
    fn test_add_https() {
        assert_eq!(
            fix_url("www.example.com"),
            Some("https://www.example.com".into())
        );
        assert_eq!(
            fix_url("m.example.com"),
            Some("https://m.example.com".into())
        );
        assert_eq!(
            fix_url("novel.html5.qq.com"),
            Some("https://novel.html5.qq.com".into())
        );
    }

    #[test]
    fn test_fix_http_colon() {
        assert_eq!(
            fix_url("http:www.example.com"),
            Some("http://www.example.com".into())
        );
    }

    #[test]
    fn test_trim_whitespace() {
        assert_eq!(
            fix_url(" https://example.com"),
            Some("https://example.com".into())
        );
    }

    #[test]
    fn test_unfixable() {
        assert_eq!(fix_url("小说合集"), None);
        assert_eq!(fix_url("DQuestQBall"), None);
        assert_eq!(fix_url(""), None);
        assert_eq!(fix_url("一个书源"), None);
    }
}
