use serde_json::Value;

use super::parser::RuleToken;

/// Extract values from a JSON Value using rule tokens.
pub fn extract(json: &Value, tokens: &[RuleToken]) -> Vec<String> {
    let mut results = Vec::new();

    if tokens.is_empty() {
        return results;
    }

    let token = &tokens[0];
    match token {
        RuleToken::JsonPath(path) => {
            let path = path.trim();
            let mut path_parts: Vec<&str> = Vec::new();
            if let Some(rest) = path.strip_prefix("$.") {
                // Split by '.', but keep [*] as separate segments
                for part in rest.split('.') {
                    if part.ends_with(']') && part.contains('[') {
                        // Split: "data[*]" → "data" and "[*]"
                        let bracket_pos = part.find('[').unwrap();
                        if bracket_pos > 0 {
                            path_parts.push(&part[..bracket_pos]);
                        }
                        path_parts.push(&part[bracket_pos..]);
                    } else {
                        path_parts.push(part);
                    }
                }
            } else if let Some(rest) = path.strip_prefix("$[")
                && (rest.starts_with("*]") || rest.starts_with('*')) {
                    let after_bracket = rest.split(']').nth(1).unwrap_or("");
                    path_parts = after_bracket.split('.').filter(|p| !p.is_empty()).collect();
                    if let Value::Array(arr) = json {
                        for item in arr {
                            collect_values(item, &path_parts, &mut results);
                        }
                        return results;
                    }
                }

            collect_values(json, &path_parts, &mut results);
        }
        RuleToken::DataPath(parts) => {
            let mut current = json;
            for part in parts {
                if let Some(v) = current.get(part) {
                    current = v;
                } else {
                    return results;
                }
            }
            if let Value::Array(arr) = current {
                for item in arr {
                    results.push(stringify_value(item));
                }
            } else {
                results.push(stringify_value(current));
            }
        }
        _ => {}
    }

    results
}

fn collect_values(value: &Value, path_parts: &[&str], results: &mut Vec<String>) {
    if path_parts.is_empty() {
        results.push(stringify_value(value));
        return;
    }

    let head = path_parts[0];
    let tail = &path_parts[1..];

    match value {
        Value::Array(arr) => {
            if head == "[*]" || head == "*" || head == "[]" {
                for item in arr {
                    collect_values(item, tail, results);
                }
            } else if let Ok(idx) = head.trim_start_matches('[').trim_end_matches(']').parse::<usize>() {
                if idx < arr.len() {
                    collect_values(&arr[idx], tail, results);
                }
            } else {
                for item in arr {
                    collect_values(item, &[head], results);
                }
            }
        }
        Value::Object(obj) => {
            if let Some(val) = obj.get(head) {
                collect_values(val, tail, results);
            }
        }
        _ => {}
    }
}

fn stringify_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        Value::Array(_) | Value::Object(_) => v.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_dsl::parser::tokenize;

    #[test]
    fn test_simple_jsonpath() {
        let json: Value = serde_json::from_str(r#"{"data":[{"name":"Book1"},{"name":"Book2"}]}"#).unwrap();
        let tokens = tokenize("$.data[*].name");
        let results = extract(&json, &tokens);
        assert_eq!(results, vec!["Book1", "Book2"]);
    }

    #[test]
    fn test_data_path() {
        let json: Value = serde_json::from_str(r#"{"books":[{"title":"Test"}]}"#).unwrap();
        let tokens = tokenize("data.books");
        let results = extract(&json, &tokens);
        assert!(!results.is_empty());
    }
}
