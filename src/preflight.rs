use crate::types::*;
use crate::url_fixer;

/// Classify a `bookSourceUrl` as valid (keep), fixable (auto-fix), or unfixable (skip).
enum UrlStatus {
    Valid(String),
    Fixable(String),
    Unfixable,
}

fn classify_url(raw: &str) -> UrlStatus {
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return UrlStatus::Valid(raw.to_string());
    }
    match url_fixer::fix_url(raw) {
        Some(fixed) => UrlStatus::Fixable(fixed),
        None => UrlStatus::Unfixable,
    }
}

fn has_search(s: &BookSource) -> bool {
    s.searchUrl
        .as_deref()
        .map_or(false, |u| !u.trim().is_empty() && u.trim() != "-" && u.trim() != "#")
}

fn has_explore(s: &BookSource) -> bool {
    s.exploreUrl.is_some()
}

/// Run the full preflight pipeline.
pub fn run(sources: Vec<BookSource>) -> PreflightOutput {
    let total_input = sources.len();

    // Step 0: filter to text (type=0) + enabled
    let (text_enabled, excluded): (Vec<_>, Vec<_>) = sources
        .into_iter()
        .partition(|s| s.bookSourceType == 0 && s.enabled);

    // Remaining working pool after URL fixing
    let mut eligible = Vec::new();
    let mut skipped = Vec::new();
    let mut explore_only = Vec::new();

    for mut source in text_enabled {
        // Step 1: classify URL
        let url = std::mem::take(&mut source.bookSourceUrl);
        match classify_url(&url) {
            UrlStatus::Valid(u) | UrlStatus::Fixable(u) => {
                source.bookSourceUrl = u;
            }
            UrlStatus::Unfixable => {
                skipped.push((source, SkipReason::BadUrl));
                continue;
            }
        }

        // Step 2: check capability
        let can_search = has_search(&source);
        let can_explore = has_explore(&source);

        if !can_search && !can_explore {
            skipped.push((source, SkipReason::NoCapability));
            continue;
        }

        if !can_search && can_explore {
            explore_only.push(source);
            continue;
        }

        // Step 3: has searchUrl but missing ruleSearch (should be ~0)
        if let Some(ref su) = source.searchUrl {
            if !su.trim().is_empty() && su.trim() != "-" && su.trim() != "#" {
                if source.ruleSearch.is_none() {
                    skipped.push((source, SkipReason::NoSearchRule));
                    continue;
                }
            }
        }

        // Step 4: check ruleContent
        if source.ruleContent.is_none() {
            skipped.push((source, SkipReason::NoContentRule));
            continue;
        }

        eligible.push(source);
    }

    // Classify eligible sources by searchUrl type
    let mut template = 0usize;
    let mut js_prefix = 0usize;
    let mut js_block = 0usize;
    let mut pure_url = 0usize;
    let mut placeholder = 0usize;

    for s in &eligible {
        let su = match s.searchUrl.as_deref() {
            Some(u) => u,
            None => continue,
        };
        if su.starts_with("@js:") {
            js_prefix += 1;
        } else if su.contains("<js>") {
            js_block += 1;
        } else if su.contains("{{") {
            template += 1;
        } else if su.trim().is_empty() || su.trim() == "-" || su.trim() == "#" {
            placeholder += 1;
        } else {
            pure_url += 1;
        }
    }

    PreflightOutput {
        total_input,
        excluded: excluded.len(),
        text_enabled: eligible.len() + skipped.len() + explore_only.len(),
        eligible,
        skipped,
        explore_only,
        breakdown: PreflightBreakdown {
            template,
            js_prefix,
            js_block,
            pure_url,
            placeholder,
        },
    }
}
