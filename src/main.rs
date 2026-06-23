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

        /// Limit to first N sources (for quick tests)
        #[arg(long)]
        limit: Option<usize>,

        #[arg(long, default_value = "data/config.toml")]
        config: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Preflight => {
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
            concurrency, timeout, no_node, force, retry_missed, limit, config: config_path,
        } => {
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

            tracing::info!("Starting test: concurrency={}, timeout={}s", test_config.concurrency, test_config.timeout_secs);

            let rt = tokio::runtime::Runtime::new()?;
            let summary = rt.block_on(tester::run(eligible.clone(), test_config, &cli.output))?;

            println!("\n=== Test Summary ===");
            println!("Total tested:  {}", summary.total);
            println!("Passed:        {}", summary.passed);
            println!("Failed:        {}", summary.failed);
            println!("JS API (skip): {}", summary.js_api.len());

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
            }

            if !summary.js_api.is_empty() {
                let js_api_path = cli.output.join("js_api.json");
                std::fs::write(&js_api_path, serde_json::to_string_pretty(&summary.js_api)?)?;
            }
        }
    }

    Ok(())
}
