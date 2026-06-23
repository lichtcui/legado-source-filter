#![allow(dead_code)]

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use legado_source_filter::*;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "legado-source-filter", about = "筛选 Legado 可用书源")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to the book sources JSON file
    #[arg(short, long, default_value = "data/b778fe6b.json")]
    input: PathBuf,

    /// Output directory
    #[arg(short, long, default_value = "output")]
    output: PathBuf,
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

        #[arg(long, default_value = "data/config.toml")]
        config: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("html5ever=off".parse().unwrap())
            .add_directive(tracing::Level::ERROR.into()))
        .init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Preflight => {
            ensure_fresh_source(&cli.input, &cli.output)?;
            tracing::info!("Loading sources from {}", cli.input.display());
            let file = std::fs::File::open(&cli.input)?;
            let reader = std::io::BufReader::new(file);
            let sources: Vec<types::BookSource> = serde_json::from_reader(reader)?;
            tracing::info!("Loaded {} sources", sources.len());

            tracing::info!("Running preflight...");
            let output = preflight::run(sources);

            tracing::info!(
                "Preflight complete: {} eligible, {} skipped, {} explore-only",
                output.eligible.len(),
                output.skipped.len(),
                output.explore_only.len()
            );

            reporter::write_outputs(&cli.output, &output)?;

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
        Commands::Test {
            concurrency, timeout, no_node, force, retry_missed, limit, config: config_path, rounds,
        } => {
            ensure_fresh_source(&cli.input, &cli.output)?;
            let config_content = std::fs::read_to_string(config_path)?;
            let config_toml: toml::Value = config_content.parse()?;

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

            let test_config = tester::TestConfig {
                concurrency: *concurrency,
                timeout_secs: *timeout,
                test_books,
                generic_keywords: vec!["重生".into(), "系统".into(), "穿越".into()],
                no_node: *no_node,
                force: *force,
                retry_missed: *retry_missed,
            };

            if *force {
                let cache_path = cli.output.join("test_cache.db");
                if cache_path.exists() {
                    std::fs::remove_file(&cache_path)?;
                }
            }

            let eligible_path = cli.output.join("eligible.json");
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

            let rt = tokio::runtime::Runtime::new()?;

            let rounds = (*rounds).max(1);
            for round in 1..=rounds {
                let mut round_config = test_config.clone();
                if round > 1 {
                    round_config.retry_missed = true;
                }

                // For rounds > 1, only retry network_error sources (not builder_error or no_results)
                let sources_to_test = if round > 1 {
                    let db_path = cli.output.join("test_cache.db");
                    filter_retryable(&eligible, &db_path)
                } else {
                    eligible.clone()
                };

                tracing::info!(
                    "Starting round {}/{}: sources={}, concurrency={}, timeout={}s",
                    round, rounds, sources_to_test.len(), round_config.concurrency, round_config.timeout_secs
                );

                let summary = rt.block_on(tester::run(sources_to_test, round_config, &cli.output))?;

                println!("\n=== Round {}/{} Summary ===", round, rounds);
                println!("Total:  {}", summary.total);
                println!("Passed: {}", summary.passed);
                println!("Failed: {}", summary.failed);
                println!("JS API: {}", summary.js_api.len());

                // Mark dead domains before the next round
                if round < rounds {
                    let db_path = cli.output.join("test_cache.db");
                    mark_dead_domains(&eligible, &db_path);
                }
            }

            // ── Final cumulative summary across all rounds ──
            {
                let db_path = cli.output.join("test_cache.db");
                let cache = db::TestCache::new(&db_path).ok();
                if let Some(ref cache) = cache {
                    let final_passed = eligible.iter().filter(|s| {
                        cache.check(&s.bookSourceUrl, &s.bookSourceName).ok().flatten()
                            .map_or(false, |(st, _)| st == "passed")
                    }).count();
                    let final_failed = eligible.iter().filter(|s| {
                        cache.check(&s.bookSourceUrl, &s.bookSourceName).ok().flatten()
                            .map_or(false, |(st, _)| st == "failed")
                    }).count();
                    let final_js_api = eligible.iter().filter(|s| {
                        cache.check(&s.bookSourceUrl, &s.bookSourceName).ok().flatten()
                            .map_or(false, |(st, _)| st == "js_api")
                    }).count();
                    let final_dead = eligible.iter().filter(|s| {
                        cache.check(&s.bookSourceUrl, &s.bookSourceName).ok().flatten()
                            .map_or(false, |(st, _)| st == "dead_domain")
                    }).count();
                    let final_skipped = eligible.iter().filter(|s| {
                        cache.check(&s.bookSourceUrl, &s.bookSourceName).ok().flatten()
                            .map_or(false, |(st, _)| st == "skipped")
                    }).count();
                    println!("\n=== Final Summary ({} rounds) ===", rounds);
                    println!("Passed:      {} ({:.1}%)", final_passed, final_passed as f64 / eligible.len() as f64 * 100.0);
                    println!("Dead domain: {} ({:.1}%)", final_dead, final_dead as f64 / eligible.len() as f64 * 100.0);
                    println!("Failed:      {} ({:.1}%)", final_failed, final_failed as f64 / eligible.len() as f64 * 100.0);
                    println!("JS API:      {}", final_js_api);
                    println!("Skipped:     {}", final_skipped);
                    println!("Untested:    {}", eligible.len() - final_passed - final_dead - final_failed - final_js_api - final_skipped);
                }
            }

            // ── Generate final output from cache ──
            let db_path = cli.output.join("test_cache.db");
            let cache = db::TestCache::new(&db_path).ok();

            if let Some(ref cache) = cache {
                let passed_sources: Vec<_> = eligible.iter()
                    .filter(|s| {
                        cache.check(&s.bookSourceUrl, &s.bookSourceName)
                            .ok()
                            .flatten()
                            .map_or(false, |(st, _)| st == "passed")
                    })
                    .cloned()
                    .collect();

                if !passed_sources.is_empty() {
                    let filtered_path = cli.output.join("filtered.json");
                    std::fs::write(&filtered_path, serde_json::to_string_pretty(&passed_sources)?)?;
                    tracing::info!("Wrote {} passed sources", passed_sources.len());
                }

                let missed_sources: Vec<_> = eligible.iter()
                    .filter(|s| {
                        cache.check(&s.bookSourceUrl, &s.bookSourceName)
                            .ok()
                            .flatten()
                            .map_or(false, |(st, _)| st == "failed" || st == "skipped")
                    })
                    .cloned()
                    .collect();

                if !missed_sources.is_empty() {
                    let missed_path = cli.output.join("missed.json");
                    std::fs::write(&missed_path, serde_json::to_string_pretty(&missed_sources)?)?;
                    tracing::info!("Wrote {} missed sources", missed_sources.len());
                }

                // Also write JS API sources (read from cache, reliable across rounds)
                let js_api_sources: Vec<_> = eligible.iter()
                    .filter(|s| {
                        cache.check(&s.bookSourceUrl, &s.bookSourceName)
                            .ok()
                            .flatten()
                            .map_or(false, |(st, _)| st == "js_api")
                    })
                    .cloned()
                    .collect();

                if !js_api_sources.is_empty() {
                    let js_api_path = cli.output.join("js_api.json");
                    std::fs::write(&js_api_path, serde_json::to_string_pretty(&js_api_sources)?)?;
                    tracing::info!("Wrote {} JS API sources", js_api_sources.len());
                }
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
        if status == "failed" {
            let _ = cache.save(&src.bookSourceUrl, &src.bookSourceName, "dead_domain", reason.as_deref(), 0);
        }
    }
}

/// For rounds > 1, only include sources worth retrying:
/// - Not yet cached (untested)
/// - Cached as "failed" with reason "network_error" (transient, could succeed next time)
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
fn ensure_fresh_source(input_path: &std::path::Path, output_dir: &std::path::Path) -> anyhow::Result<()> {
    let source_url_path = output_dir.join(".source_url");

    // Fetch aoaostar index page (small, ~15 KB)
    let html = fetch_text("https://legado.aoaostar.com")?;
    let remote_url = discover_source_url(&html)?;
    let remote_name = remote_url.rsplit('/').next().unwrap_or("");

    // Compare with the last downloaded URL
    let last_url = std::fs::read_to_string(&source_url_path).ok();
    if let Some(ref last) = last_url {
        if last.trim() == remote_url && input_path.exists() {
            tracing::info!("书源已是最新: {}", remote_name);
            return Ok(());
        }
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

fn discover_source_url(html: &str) -> anyhow::Result<String> {
    let section_start = html.find("全量书源")
        .ok_or_else(|| anyhow::anyhow!("未在页面中找到「全量书源」信息"))?;
    let section = &html[section_start..];
    let link_start = section.find("<a href=\"")
        .ok_or_else(|| anyhow::anyhow!("未找到书源链接"))?;
    let url_start = link_start + "<a href=\"".len();
    let url_end = section[url_start..].find('\"')
        .ok_or_else(|| anyhow::anyhow!("书源链接格式错误"))?;
    Ok(section[url_start..url_start + url_end].to_string())
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
