use std::fs;
use std::io::Write;
use std::path::Path;

use crate::types::*;

pub fn write_outputs(
    output_dir: &Path,
    output: &PreflightOutput,
    json_output: bool,
) -> anyhow::Result<()> {
    fs::create_dir_all(output_dir)?;

    // ── eligible.json ──
    let eligible_path = output_dir.join("eligible.json");
    let eligible_json = serde_json::to_string_pretty(&output.eligible)?;
    fs::write(&eligible_path, &eligible_json)?;

    // ── skipped.json ──
    let skipped_path = output_dir.join("skipped.json");
    let skipped_entries: Vec<_> = output
        .skipped
        .iter()
        .map(|(source, reason)| {
            serde_json::json!({
                "bookSourceName": source.bookSourceName,
                "bookSourceUrl": source.bookSourceUrl,
                "bookSourceGroup": source.bookSourceGroup,
                "skip_reason": reason,
            })
        })
        .collect();
    let skipped_json = serde_json::to_string_pretty(&skipped_entries)?;
    fs::write(&skipped_path, &skipped_json)?;

    // ── explore_only.json ──
    let explore_path = output_dir.join("explore_only.json");
    let explore_json = serde_json::to_string_pretty(&output.explore_only)?;
    fs::write(&explore_path, &explore_json)?;

    // ── report.txt ──
    let report_path = output_dir.join("report.txt");
    let mut report = fs::File::create(&report_path)?;

    writeln!(report, "=== Legado 书源预检报告 ===")?;
    writeln!(report)?;
    writeln!(report, "全量输入:     {} 个", output.total_input)?;
    writeln!(report, "排除(非文字/禁用): {} 个", output.excluded)?;
    writeln!(report, "文字+启用:     {} 个", output.text_enabled)?;
    writeln!(report)?;

    // Group skipped by reason
    use std::collections::HashMap;
    let mut skip_counts: HashMap<String, usize> = HashMap::new();
    for (_, reason) in &output.skipped {
        let key = serde_json::to_string(reason).unwrap_or_default();
        *skip_counts.entry(key).or_insert(0) += 1;
    }

    writeln!(report, "--- 跳过明细 ---")?;
    for (reason, count) in &skip_counts {
        writeln!(report, "  {:<20} {}", reason, count)?;
    }
    writeln!(report, "  跳过合计:       {}", output.skipped.len())?;
    writeln!(report, "  仅探索:         {}", output.explore_only.len())?;
    writeln!(report, "  待测试(eligible): {}", output.eligible.len())?;
    writeln!(report)?;

    writeln!(report, "--- 搜索 URL 类型分布 ---")?;
    let b = &output.breakdown;
    writeln!(report, "  {{key}} 模板:   {}", b.template)?;
    writeln!(report, "  @js: 前缀:     {}", b.js_prefix)?;
    writeln!(report, "  <js> 内嵌:     {}", b.js_block)?;
    writeln!(report, "  纯 URL:        {}", b.pure_url)?;
    writeln!(report, "  占位符:        {}", b.placeholder)?;
    writeln!(report, "  合计:          {}", b.template + b.js_prefix + b.js_block + b.pure_url + b.placeholder)?;

    // ── report.json (structured, only in JSON mode) ──
    if json_output {
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
        let json_path = output_dir.join("report.json");
        fs::write(&json_path, serde_json::to_string_pretty(&report_json)?)?;
    }

    Ok(())
}
