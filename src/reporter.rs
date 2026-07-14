use std::fs;
use std::path::Path;

use crate::db::TestCache;
use crate::types::*;

pub fn write_outputs(
    output_dir: &Path,
    db_path: &Path,
    output: &PreflightOutput,
) -> anyhow::Result<()> {
    fs::create_dir_all(output_dir)?;
    let cache = TestCache::new(db_path)?;

    // ── eligible → DB ──
    let eligible_json = serde_json::to_string(&output.eligible)?;
    cache.save_meta("eligible", &eligible_json)?;

    // ── explore_only.json ──
    if !output.explore_only.is_empty() {
        let explore_path = output_dir.join("explore_only.json");
        let explore_json = serde_json::to_string_pretty(&output.explore_only)?;
        fs::write(&explore_path, &explore_json)?;
    }

    // ── report → DB ──
    use std::collections::HashMap;
    let mut skip_counts: HashMap<String, usize> = HashMap::new();
    for (_, reason) in &output.skipped {
        let key = serde_json::to_string(reason).unwrap_or_default();
        *skip_counts.entry(key).or_insert(0) += 1;
    }
    let skip_detail: serde_json::Value = skip_counts.iter().map(|(k, v)| (k.to_string(), *v)).collect();

    let report_json = serde_json::json!({
        "summary": {
            "total_input": output.total_input,
            "excluded": output.excluded,
            "text_enabled": output.text_enabled,
            "eligible": output.eligible.len(),
            "skipped": output.skipped.len(),
            "explore_only": output.explore_only.len(),
        },
        "skip_detail": skip_detail,
        "breakdown": {
            "template": output.breakdown.template,
            "js_prefix": output.breakdown.js_prefix,
            "js_block": output.breakdown.js_block,
            "pure_url": output.breakdown.pure_url,
            "placeholder": output.breakdown.placeholder,
        },
    });
    cache.save_meta("report", &serde_json::to_string(&report_json)?)?;

    Ok(())
}
