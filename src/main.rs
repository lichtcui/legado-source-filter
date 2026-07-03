use std::path::PathBuf;

use std::io::Write;

use clap::{Parser, Subcommand};
use legado_source_filter::*;
use tracing_subscriber::EnvFilter;

const DEFAULT_CONFIG: &str = include_str!("../data/config.toml");

fn xdg_input() -> PathBuf {
    let home = || std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let base = std::env::var("XDG_DATA_HOME")
        .unwrap_or_else(|_| format!("{}/.local/share", home()));
    PathBuf::from(base).join("legado-source-filter").join("sources.json")
}

fn xdg_output() -> PathBuf {
    let home = || std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let base = std::env::var("XDG_CACHE_HOME")
        .unwrap_or_else(|_| format!("{}/.cache", home()));
    PathBuf::from(base).join("legado-source-filter")
}

#[derive(Parser)]
#[command(name = "legado-source-filter", about = "筛选 Legado 可用书源")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Emit machine-readable JSON Lines to stdout
    #[arg(long, global = true)]
    json: bool,

    /// Path to the book sources JSON file (default: XDG_DATA_HOME/legado-source-filter/sources.json)
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Output directory (default: XDG_CACHE_HOME/legado-source-filter)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run static preflight checks (no network requests)
    Preflight,

    /// Run search tests on eligible sources
    Test {
        #[arg(short, long, default_value = "50")]
        concurrency: usize,

        #[arg(short, long, default_value = "15")]
        timeout: u64,

        /// Skip JS sources (no node required)
        #[arg(long)]
        no_node: bool,

        /// Re-test all sources, ignoring cache
        #[arg(long)]
        force: bool,

        /// Only re-test previously failed sources
        #[arg(long)]
        retry_missed: bool,

        /// Number of test rounds. Failed sources are retried each round.
        /// A source passes if it succeeds in any round (handles network flakiness).
        #[arg(long, default_value = "1")]
        rounds: u32,

        /// Limit to first N sources (for quick tests)
        #[arg(long)]
        limit: Option<usize>,

        /// Path to config.toml (optional; built-in defaults used if not specified)
        #[arg(long)]
        config: Option<PathBuf>,
    },

    /// Show current pipeline status from cache (no network)
    Status,

    /// One-shot: preflight + test with N rounds
    Full {
        #[arg(short, long, default_value = "50")]
        concurrency: usize,

        #[arg(short, long, default_value = "15")]
        timeout: u64,

        /// Skip JS sources (no node required)
        #[arg(long)]
        no_node: bool,

        /// Re-test all sources, ignoring cache
        #[arg(long)]
        force: bool,

        /// Number of test rounds. Failed sources are retried each round.
        #[arg(long, default_value = "1")]
        rounds: u32,

        /// Limit to first N sources (for quick tests)
        #[arg(long)]
        limit: Option<usize>,

        /// Path to config.toml (optional; built-in defaults used if not specified)
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

/// Emit a JSON event to stdout if --json is enabled.
fn json_event(cli: &Cli, value: serde_json::Value) {
    if !cli.json {
        return;
    }
    println!("{}", serde_json::to_string(&value).unwrap());
    let _ = std::io::stdout().flush();
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if !cli.json {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env()
                .add_directive("html5ever=off".parse().unwrap())
                .add_directive(tracing::Level::ERROR.into()))
            .init();
    }

    let input_path = cli.input.clone().unwrap_or_else(xdg_input);
    let output_path = cli.output.clone().unwrap_or_else(xdg_output);

    match &cli.command {
        Commands::Status => {
            let db_path = output_path.join("test_cache.db");
            let eligible_path = output_path.join("eligible.json");

            let total = std::fs::read_to_string(&eligible_path).ok()
                .and_then(|s| serde_json::from_str::<Vec<types::BookSource>>(&s).ok())
                .map(|v| v.len());

            if let Some((n_total, cache)) = total.and_then(|t| {
                db::TestCache::new(&db_path).ok().map(|c| (t, c))
            }) {
                let rows = cache.summary().unwrap_or_default();
                let tested: usize = rows.iter().map(|(_, c)| c).sum();
                let mut counts = std::collections::BTreeMap::new();
                for (s, c) in &rows {
                    counts.insert(s.clone(), *c);
                }

                json_event(&cli, serde_json::json!({
                    "event": "status",
                    "preflight": "done",
                    "total": n_total,
                    "tested": tested,
                    "remaining": n_total.saturating_sub(tested),
                    "details": counts,
                }));

                if !cli.json {
                    println!("=== Pipeline Status ===");
                    println!("Preflight: done ({} eligible)", n_total);
                    println!("Tested:    {} / {}", tested, n_total);
                    for (status, count) in &rows {
                        println!("  {:<16} {}", status, count);
                    }
                    let remaining = n_total - tested;
                    println!("Remaining: {}", remaining);
                }
            } else if eligible_path.exists() {
                let n_total = std::fs::read_to_string(&eligible_path).ok()
                    .and_then(|s| serde_json::from_str::<Vec<types::BookSource>>(&s).ok())
                    .map_or(0, |v| v.len());

                json_event(&cli, serde_json::json!({
                    "event": "status",
                    "preflight": "done",
                    "total": n_total,
                    "tested": 0,
                    "remaining": n_total,
                }));

                if !cli.json {
                    println!("=== Pipeline Status ===");
                    println!("Preflight: done ({} eligible)", n_total);
                    println!("Test:      not started yet");
                }
            } else {
                json_event(&cli, serde_json::json!({
                    "event": "status",
                    "preflight": "not_run",
                    "total": 0,
                }));

                if !cli.json {
                    println!("=== Pipeline Status ===");
                    println!("Preflight: not run yet — run `preflight` first");
                }
            }
        }

        Commands::Preflight => {
            json_event(&cli, serde_json::json!({"event": "phase", "phase": "preflight", "status": "started"}));

            ensure_fresh_source(&input_path, &output_path)?;
            let file = std::fs::File::open(&input_path)?;
            let reader = std::io::BufReader::new(file);
            let sources: Vec<types::BookSource> = serde_json::from_reader(reader)?;

            let output = preflight::run(sources);

            reporter::write_outputs(&output_path, &output, cli.json)?;

            json_event(&cli, serde_json::json!({
                "event": "preflight_summary",
                "total_input": output.total_input,
                "excluded": output.excluded,
                "text_enabled": output.text_enabled,
                "skipped": output.skipped.len(),
                "explore_only": output.explore_only.len(),
                "eligible": output.eligible.len(),
                "breakdown": {
                    "template": output.breakdown.template,
                    "js_prefix": output.breakdown.js_prefix,
                    "js_block": output.breakdown.js_block,
                    "pure_url": output.breakdown.pure_url,
                },
            }));

            if !cli.json {
                println!("\n=== Preflight Summary ===");
                println!("Total input:      {}", output.total_input);
                println!("Excluded (non-text/disabled): {}", output.excluded);
                println!("Text + enabled:   {}", output.text_enabled);
                println!("Skipped:          {}", output.skipped.len());
                println!("Explore only:     {}", output.explore_only.len());
                println!("Eligible (test):  {}", output.eligible.len());
                let b = &output.breakdown;
                println!("  {{key}} template: {}", b.template);
                println!("  @js: prefix:     {}", b.js_prefix);
                println!("  <js> block:      {}", b.js_block);
                println!("  Pure URL:        {}", b.pure_url);
            }
        }
        Commands::Test {
            concurrency, timeout, no_node, force, retry_missed, limit, config: config_path, rounds,
        } => {
            json_event(&cli, serde_json::json!({"event": "phase", "phase": "test", "status": "started"}));
            ensure_fresh_source(&input_path, &output_path)?;

            let test_config = build_test_config(
                *concurrency, *timeout, *no_node, *force, *retry_missed, config_path,
            )?;

            if *force {
                let cache_path = output_path.join("test_cache.db");
                if cache_path.exists() {
                    std::fs::remove_file(&cache_path)?;
                }
            }

            let eligible_path = output_path.join("eligible.json");
            if !eligible_path.exists() {
                anyhow::bail!("eligible.json not found. Run `preflight` first.");
            }

            let eligible: Vec<types::BookSource> = {
                let file = std::fs::File::open(&eligible_path)?;
                let reader = std::io::BufReader::new(file);
                serde_json::from_reader(reader)?
            };

            // Apply --limit
            let eligible = if let Some(n) = limit {
                let n = (*n).min(eligible.len());
                tracing::info!("Limited to first {} sources", n);
                eligible.into_iter().take(n).collect()
            } else {
                eligible
            };

            run_test_campaign(eligible, test_config, &output_path, cli.json, *rounds)?;
        }

        Commands::Full {
            concurrency, timeout, no_node, force, limit, config: config_path, rounds,
        } => {
            json_event(&cli, serde_json::json!({"event": "phase", "phase": "full", "status": "started"}));

            // ── Phase 1: Preflight ──
            json_event(&cli, serde_json::json!({"event": "phase", "phase": "preflight", "status": "started"}));
            ensure_fresh_source(&input_path, &output_path)?;
            let file = std::fs::File::open(&input_path)?;
            let reader = std::io::BufReader::new(file);
            let sources: Vec<types::BookSource> = serde_json::from_reader(reader)?;
            let preflight_out = preflight::run(sources);
            reporter::write_outputs(&output_path, &preflight_out, cli.json)?;

            json_event(&cli, serde_json::json!({
                "event": "preflight_summary",
                "total_input": preflight_out.total_input,
                "eligible": preflight_out.eligible.len(),
                "skipped": preflight_out.skipped.len(),
                "explore_only": preflight_out.explore_only.len(),
            }));

            if !cli.json {
                println!("\n=== Preflight ===");
                println!("Eligible: {}", preflight_out.eligible.len());
            }

            if preflight_out.eligible.is_empty() {
                anyhow::bail!("No eligible sources after preflight");
            }

            // full 命令默认从头跑：清除旧测试缓存和衍生输出
            let cache_path = output_path.join("test_cache.db");
            if cache_path.exists() {
                std::fs::remove_file(&cache_path)?;
            }
            for f in &["filtered.json", "missed.json", "js_api.json"] {
                let p = output_path.join(f);
                if p.exists() {
                    std::fs::remove_file(&p)?;
                }
            }

            // ── Phase 2: Build test config ──
            let test_config = build_test_config(
                *concurrency, *timeout, *no_node, *force, false, config_path,
            )?;

            let eligible = if let Some(n) = limit {
                let n = (*n).min(preflight_out.eligible.len());
                preflight_out.eligible.into_iter().take(n).collect()
            } else {
                preflight_out.eligible
            };

            // ── Phase 3: Test campaign ──
            json_event(&cli, serde_json::json!({"event": "phase", "phase": "test", "status": "started"}));
            run_test_campaign(eligible, test_config, &output_path, cli.json, *rounds)?;
        }
    }

    Ok(())
}

/// Build a TestConfig from config.toml.
fn build_test_config(
    concurrency: usize,
    timeout_secs: u64,
    no_node: bool,
    force: bool,
    retry_missed: bool,
    config_path: &Option<PathBuf>,
) -> anyhow::Result<tester::TestConfig> {
    let config_toml = load_config(config_path)?;

    let mut test_books = Vec::new();
    if let Some(searches) = config_toml.get("search").and_then(|v| v.as_array()) {
        for entry in searches {
            let name = entry.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if !name.is_empty() {
                test_books.push(tester::TestBook {
                    name,
                    author: entry.get("author").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    domain_hint: None,
                });
            }
        }
    }

    Ok(tester::TestConfig {
        concurrency,
        timeout_secs,
        test_books,
        generic_keywords: vec!["重生".into(), "系统".into(), "穿越".into()],
        no_node,
        force,
        retry_missed,
    })
}

/// Run the full test campaign for an eligible set: N rounds of testing,
/// dead-domain marking between rounds, final cumulative summary, and
/// output file generation (filtered.json / missed.json / js_api.json).
fn run_test_campaign(
    eligible: Vec<types::BookSource>,
    config: tester::TestConfig,
    output_path: &std::path::Path,
    json_output: bool,
    rounds: u32,
) -> anyhow::Result<()> {
    if eligible.is_empty() {
        if json_output {
            println!("{}", serde_json::to_string(&serde_json::json!({
                "event": "round_summary",
                "total_rounds": 0, "total": 0, "passed": 0, "failed": 0, "cached": 0, "js_api": 0,
                "note": "no eligible sources",
            })).unwrap());
        }
        return Ok(());
    }
    if config.concurrency < 1 {
        anyhow::bail!("concurrency must be at least 1");
    }
    let rt = tokio::runtime::Runtime::new()?;
    let rounds = rounds.max(1);

    for round in 1..=rounds {
        let mut round_config = config.clone();
        if round > 1 {
            round_config.retry_missed = true;
        }

        let sources_to_test = if round > 1 {
            let db_path = output_path.join("test_cache.db");
            filter_retryable(&eligible, &db_path)
        } else {
            eligible.clone()
        };

        if sources_to_test.is_empty() {
            let event = serde_json::json!({
                "event": "round_summary",
                "round": round,
                "total_rounds": rounds,
                "total": 0, "passed": 0, "failed": 0, "js_api": 0,
                "note": "nothing to retry",
            });
            if json_output {
                println!("{}", serde_json::to_string(&event).unwrap());
            } else {
                println!("\n=== Round {}/{} === nothing to retry", round, rounds);
            }
            break;
        }

        if json_output {
            println!("{}", serde_json::to_string(&serde_json::json!({
                "event": "round_start",
                "round": round,
                "total_rounds": rounds,
                "sources": sources_to_test.len(),
            })).unwrap());
        } else {
            tracing::info!(
                "Starting round {}/{}: sources={}, concurrency={}, timeout={}s",
                round, rounds, sources_to_test.len(),
                round_config.concurrency, round_config.timeout_secs
            );
        }

        let summary = rt.block_on(tester::run(sources_to_test, round_config, output_path, json_output))?;

        let event = serde_json::json!({
            "event": "round_summary",
            "round": round,
            "total_rounds": rounds,
            "total": summary.total,
            "passed": summary.passed,
            "failed": summary.failed,
            "cached": summary.cached,
            "js_api": summary.js_api.len(),
        });
        if json_output {
            println!("{}", serde_json::to_string(&event).unwrap());
        } else {
            println!("\n=== Round {}/{} Summary ===", round, rounds);
            println!("Total:  {}", summary.total);
            println!("Passed: {}", summary.passed);
            println!("Failed: {}", summary.failed);
            println!("Cached: {}", summary.cached);
            println!("JS API: {}", summary.js_api.len());
        }

        if round < rounds {
            let db_path = output_path.join("test_cache.db");
            mark_dead_domains(&eligible, &db_path);
        }
    }

    // ── Final cumulative summary across all rounds ──
    {
        let db_path = output_path.join("test_cache.db");
        if let Ok(cache) = db::TestCache::new(&db_path) {
            let final_passed = eligible.iter().filter(|s| {
                cache.check(&s.bookSourceUrl, &s.bookSourceName).ok().flatten()
                    .is_some_and(|(st, _)| st == "passed")
            }).count();
            let final_failed = eligible.iter().filter(|s| {
                cache.check(&s.bookSourceUrl, &s.bookSourceName).ok().flatten()
                    .is_some_and(|(st, _)| st == "failed")
            }).count();
            let final_js_api = eligible.iter().filter(|s| {
                cache.check(&s.bookSourceUrl, &s.bookSourceName).ok().flatten()
                    .is_some_and(|(st, _)| st == "js_api")
            }).count();
            let final_dead = eligible.iter().filter(|s| {
                cache.check(&s.bookSourceUrl, &s.bookSourceName).ok().flatten()
                    .is_some_and(|(st, _)| st == "dead_domain")
            }).count();
            let final_skipped = eligible.iter().filter(|s| {
                cache.check(&s.bookSourceUrl, &s.bookSourceName).ok().flatten()
                    .is_some_and(|(st, _)| st == "skipped")
            }).count();
            let untested = eligible.len() - final_passed - final_dead - final_failed - final_js_api - final_skipped;

            let final_event = serde_json::json!({
                "event": "final_summary",
                "rounds": rounds,
                "total": eligible.len(),
                "passed": final_passed,
                "dead_domain": final_dead,
                "failed": final_failed,
                "js_api": final_js_api,
                "skipped": final_skipped,
                "untested": untested,
            });
            if json_output {
                println!("{}", serde_json::to_string(&final_event).unwrap());
            }

            // Always write report.json (structured output)
            let report_json = serde_json::json!({
                "summary": {
                    "rounds": rounds,
                    "total": eligible.len(),
                    "passed": final_passed,
                    "dead_domain": final_dead,
                    "failed": final_failed,
                    "js_api": final_js_api,
                    "skipped": final_skipped,
                    "untested": untested,
                }
            });
            let report_path = output_path.join("report.json");
            let _ = std::fs::write(&report_path, serde_json::to_string_pretty(&report_json)?);

            if !json_output {
                println!("\n=== Final Summary ({} rounds) ===", rounds);
                println!("Passed:      {} ({:.1}%)", final_passed, final_passed as f64 / eligible.len() as f64 * 100.0);
                println!("Dead domain: {} ({:.1}%)", final_dead, final_dead as f64 / eligible.len() as f64 * 100.0);
                println!("Failed:      {} ({:.1}%)", final_failed, final_failed as f64 / eligible.len() as f64 * 100.0);
                println!("JS API:      {}", final_js_api);
                println!("Skipped:     {}", final_skipped);
                println!("Untested:    {}", untested);
            }
        }
    }

    // ── Generate final output files from cache ──
    {
        let db_path = output_path.join("test_cache.db");
        if let Ok(cache) = db::TestCache::new(&db_path) {
            let passed_sources: Vec<_> = eligible.iter()
                .filter(|s| {
                    cache.check(&s.bookSourceUrl, &s.bookSourceName)
                        .ok()
                        .flatten()
                        .is_some_and(|(st, _)| st == "passed")
                })
                .cloned()
                .collect();
            if !passed_sources.is_empty() {
                let filtered_path = output_path.join("filtered.json");
                std::fs::write(&filtered_path, serde_json::to_string_pretty(&passed_sources)?)?;
                tracing::info!("Wrote {} passed sources to filtered.json", passed_sources.len());
            }

            let missed_sources: Vec<_> = eligible.iter()
                .filter(|s| {
                    cache.check(&s.bookSourceUrl, &s.bookSourceName)
                        .ok()
                        .flatten()
                        .is_some_and(|(st, _)| st == "failed" || st == "skipped")
                })
                .cloned()
                .collect();
            if !missed_sources.is_empty() {
                let missed_path = output_path.join("missed.json");
                std::fs::write(&missed_path, serde_json::to_string_pretty(&missed_sources)?)?;
                tracing::info!("Wrote {} missed sources to missed.json", missed_sources.len());
            }

            let js_api_sources: Vec<_> = eligible.iter()
                .filter(|s| {
                    cache.check(&s.bookSourceUrl, &s.bookSourceName)
                        .ok()
                        .flatten()
                        .is_some_and(|(st, _)| st == "js_api")
                })
                .cloned()
                .collect();
            if !js_api_sources.is_empty() {
                let js_api_path = output_path.join("js_api.json");
                std::fs::write(&js_api_path, serde_json::to_string_pretty(&js_api_sources)?)?;
                tracing::info!("Wrote {} JS API sources to js_api.json", js_api_sources.len());
            }
        }
    }

    Ok(())
}

/// Analyze cache after a round: if every tested source on a domain has "failed"
/// (none passed), mark all its failed sources as "dead_domain" so subsequent
/// rounds skip them — retrying a dead site is wasted effort.
fn mark_dead_domains(eligible: &[types::BookSource], db_path: &std::path::Path) {
    let Ok(cache) = db::TestCache::new(db_path) else { return; };

    let mut domain_has_passed = std::collections::HashSet::new();

    // First pass: find domains with at least one passed source
    for src in eligible {
        let Ok(Some((status, _))) = cache.check(&src.bookSourceUrl, &src.bookSourceName) else { continue; };
        let domain = src.bookSourceUrl.split('/').nth(2).unwrap_or("").to_lowercase();
        if domain.is_empty() { continue; }
        if status == "passed" {
            domain_has_passed.insert(domain);
        }
    }

    // Second pass: mark failed sources on dead domains
    for src in eligible {
        let domain = src.bookSourceUrl.split('/').nth(2).unwrap_or("").to_lowercase();
        if domain.is_empty() || domain_has_passed.contains(&domain) { continue; }

        let Ok(Some((status, reason))) = cache.check(&src.bookSourceUrl, &src.bookSourceName) else { continue; };
        if status == "failed"
            && let Err(e) = cache.save(&src.bookSourceUrl, &src.bookSourceName, "dead_domain", reason.as_deref(), 0)
        {
            tracing::warn!("cache save (dead_domain) failed for {}: {}", src.bookSourceName, e);
        }
    }
}

/// For rounds > 1, only include sources worth retrying:
///   - Not yet cached (untested)
///   - Cached as "failed" with reason "network_error" (transient, could succeed next time)
///
/// All other statuses (passed, js_api, dead_domain, builder_error, no_results) are skipped.
fn filter_retryable(eligible: &[types::BookSource], db_path: &std::path::Path) -> Vec<types::BookSource> {
    let Ok(cache) = db::TestCache::new(db_path) else { return eligible.to_vec(); };

    eligible.iter()
        .filter(|src| {
            let Ok(Some((status, reason))) = cache.check(&src.bookSourceUrl, &src.bookSourceName) else {
                return true; // Not in cache yet → include
            };
            status == "failed" && reason.as_deref() == Some("network_error")
        })
        .cloned()
        .collect()
}

/// Ensure the local book source JSON is up to date.
/// Fetches the aoaostar index page, discovers the latest "全量书源" URL,
/// downloads if the remote filename has changed, and clears stale caches.
/// If the remote is unreachable, silently falls back to the local file.
fn ensure_fresh_source(input_path: &std::path::Path, output_dir: &std::path::Path) -> anyhow::Result<()> {
    let source_url_path = output_dir.join(".source_url");

    // Ensure data/ and output/ directories exist (for cargo install scenario)
    if let Some(parent) = input_path.parent()
        && !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    std::fs::create_dir_all(output_dir)?;

    // If we already have a local file, try remote update but don't fail if unreachable
    let has_local = input_path.exists();

    let html = match fetch_text("https://legado.aoaostar.com") {
        Ok(h) => h,
        Err(e) => {
            if has_local {
                println!("无法连接书源服务器，使用本地缓存");
                return Ok(());
            }
            return Err(e);
        }
    };

    let remote_url = discover_source_url(&html)?;
    let remote_name = remote_url.rsplit('/').next().unwrap_or("");

    // Compare with the last downloaded URL
    let last_url = std::fs::read_to_string(&source_url_path).ok();
    if let Some(ref last) = last_url
        && last.trim() == remote_url && has_local {
            tracing::info!("书源已是最新: {}", remote_name);
            return Ok(());
        }

    // Download new JSON to a temp file, then atomically replace
    tracing::info!("发现新书源: {}，正在下载...", remote_name);
    let temp_path = input_path.with_extension("tmp");
    download_to_file(&remote_url, &temp_path)?;
    std::fs::rename(&temp_path, input_path)?;
    std::fs::write(&source_url_path, &remote_url)?;

    tracing::info!("书源已更新 ({}), 清除旧缓存", remote_name);
    clear_output_cache(output_dir);
    Ok(())
}

fn fetch_text(url: &str) -> anyhow::Result<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} fetching {}", resp.status(), url);
    }
    Ok(resp.text()?)
}

fn download_to_file(url: &str, path: &std::path::Path) -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} downloading {}", resp.status(), url);
    }
    let bytes = resp.bytes()?;
    std::fs::write(path, &bytes)?;
    Ok(())
}

fn discover_source_url(html_str: &str) -> anyhow::Result<String> {
    use scraper::{Html, Selector};
    let doc = Html::parse_document(html_str);
    let a_sel = Selector::parse("a[href]").unwrap();

    for elem in doc.root_element().select(&a_sel) {
        let text: String = elem.text().collect();
        if text.contains("全量书源")
            && let Some(href) = elem.value().attr("href")
        {
            return Ok(href.to_string());
        }
    }

    anyhow::bail!("未在页面中找到全量书源链接");
}

fn clear_output_cache(output_dir: &std::path::Path) {
    for entry in &["test_cache.db", "eligible.json", "filtered.json", "missed.json",
                   "skipped.json", "explore_only.json", "js_api.json", "report.txt"] {
        let path = output_dir.join(entry);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Load config.toml: from explicit path, default `data/config.toml`, or embedded default.
fn load_config(config: &Option<PathBuf>) -> anyhow::Result<toml::Value> {
    let content = if let Some(path) = config {
        std::fs::read_to_string(path)?
    } else {
        let default_path = PathBuf::from("data/config.toml");
        if default_path.exists() {
            std::fs::read_to_string(&default_path)?
        } else {
            DEFAULT_CONFIG.to_string()
        }
    };
    Ok(content.parse()?)
}
