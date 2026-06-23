/// Integration test: verify preflight pipeline produces expected numbers
#[test]
fn test_preflight_pipeline() {
    let path = std::path::Path::new("data/b778fe6b.json");
    if !path.exists() {
        eprintln!("Skipping integration test: data file not found");
        return;
    }

    let file = std::fs::File::open(path).unwrap();
    let reader = std::io::BufReader::new(file);
    let sources: Vec<legado_source_filter::types::BookSource> =
        serde_json::from_reader(reader).unwrap();

    assert_eq!(sources.len(), 3911, "Should have 3911 total sources");

    let output = legado_source_filter::preflight::run(sources);

    assert_eq!(output.text_enabled, 3680, "Text+enabled should be 3680");
    assert_eq!(output.eligible.len(), 2957, "Eligible should be 2957");

    assert_eq!(
        output.skipped.len() + output.explore_only.len() + output.eligible.len(),
        output.text_enabled,
        "Pipeline counts should sum to text_enabled"
    );

    let b = &output.breakdown;
    assert_eq!(
        b.template + b.js_prefix + b.js_block + b.pure_url + b.placeholder,
        output.eligible.len(),
        "Search URL types should sum to eligible"
    );
}

/// Test url_fixer handles known patterns
#[test]
fn test_url_fixer_cases() {
    use legado_source_filter::url_fixer::fix_url;

    assert_eq!(fix_url("https://example.com"), Some("https://example.com".into()));
    assert_eq!(fix_url("www.example.com"), Some("https://www.example.com".into()));
    assert_eq!(fix_url("m.xxbiqudu.com"), Some("https://m.xxbiqudu.com".into()));
    assert_eq!(
        fix_url("[图片]http://www.sfacg.com"),
        Some("http://www.sfacg.com".into())
    );
    assert_eq!(fix_url("小说合集"), None);
    assert_eq!(fix_url("DQuestQBall"), None);
}

/// Test rule_dsl parser handles key patterns
#[test]
fn test_rule_dsl_parser() {
    use legado_source_filter::rule_dsl::parser::tokenize;

    let t = tokenize("class.item@tag.li@text");
    assert_eq!(t.len(), 3);

    let t = tokenize("class.item.0@tag.a@href");
    assert!(t.len() >= 4);

    let t = tokenize("$.data[*].name");
    assert_eq!(t.len(), 1);

    let t = tokenize("class.a@text||class.b@text");
    assert!(t.iter().any(|tok| {
        matches!(tok, legado_source_filter::rule_dsl::parser::RuleToken::Fallback)
    }));
}

/// Test HTML extraction
#[test]
fn test_html_extractor() {
    use legado_source_filter::rule_dsl::html_extractor::extract;
    use legado_source_filter::rule_dsl::parser::tokenize;

    let html = r#"<div class="item"><a href="/book/1">Book 1</a></div>"#;
    let tokens = tokenize("class.item@tag.a@text");
    let results = extract(html, &tokens);
    assert_eq!(results, vec!["Book 1"]);
}

/// Test JSON extraction
#[test]
fn test_json_extractor() {
    use legado_source_filter::rule_dsl::json_extractor::extract;
    use legado_source_filter::rule_dsl::parser::tokenize;

    let json = serde_json::json!({"data": [{"name": "Book1"}, {"name": "Book2"}]});
    let tokens = tokenize("$.data[*].name");
    let results = extract(&json, &tokens);
    assert_eq!(results, vec!["Book1", "Book2"]);
}

/// Test search URL template replacement
#[test]
fn test_search_url_templates() {
    use legado_source_filter::search_url::build_request;
    use legado_source_filter::types::BookSource;

    let source = BookSource {
        bookSourceName: "test".into(),
        bookSourceUrl: "https://example.com".into(),
        bookSourceType: 0,
        enabled: true,
        searchUrl: Some("/search?keyword={{key}}&page={{page}}".into()),
        ..Default::default()
    };

    let spec = build_request(&source, "重生").unwrap();
    assert_eq!(spec.url, "https://example.com/search?keyword=重生&page=1");
    assert_eq!(spec.method, "GET");
}
