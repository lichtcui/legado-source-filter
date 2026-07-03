use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[allow(non_snake_case)]
pub struct BookSource {
    pub bookSourceName: String,
    pub bookSourceUrl: String,
    pub bookSourceType: i64,
    pub bookSourceGroup: Option<String>,
    pub bookSourceComment: Option<String>,
    pub bookUrlPattern: Option<String>,

    #[serde(default)]
    pub customOrder: i64,

    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub enabledCookieJar: bool,

    #[serde(default)]
    pub enabledExplore: bool,

    #[serde(default)]
    pub enabledReview: bool,

    #[serde(default)]
    pub exploreUrl: Option<serde_json::Value>,

    pub header: Option<String>,
    pub loginUrl: Option<String>,
    pub loginUi: Option<String>,
    pub loginCheckJs: Option<String>,

    #[serde(default)]
    pub lastUpdateTime: i64,

    #[serde(default)]
    pub respondTime: i64,

    #[serde(default)]
    pub weight: i64,

    #[serde(default)]
    pub concurrentRate: Option<serde_json::Value>,

    pub coverDecodeJs: Option<String>,
    pub exploreScreen: Option<String>,
    pub jsLib: Option<String>,
    pub variableComment: Option<String>,

    pub searchUrl: Option<String>,
    pub ruleSearch: Option<SearchRule>,
    pub ruleBookInfo: Option<BookInfoRule>,
    pub ruleContent: Option<ContentRule>,
    pub ruleToc: Option<TocRule>,
    pub ruleExplore: Option<ExploreRule>,
    pub ruleReview: Option<serde_json::Value>,
}

fn default_true() -> bool {
    true
}

// ── Rule structs ──────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[allow(non_snake_case)]
pub struct SearchRule {
    pub bookList: Option<String>,
    pub name: Option<String>,
    pub author: Option<String>,
    pub bookUrl: Option<String>,
    pub coverUrl: Option<String>,
    pub intro: Option<String>,
    pub kind: Option<String>,
    pub lastChapter: Option<String>,
    pub wordCount: Option<String>,
    pub checkKeyWord: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[allow(non_snake_case)]
pub struct BookInfoRule {
    pub name: Option<String>,
    pub author: Option<String>,
    pub kind: Option<String>,
    pub wordCount: Option<String>,
    pub lastChapter: Option<String>,
    pub intro: Option<String>,
    pub coverUrl: Option<String>,
    pub tocUrl: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[allow(non_snake_case)]
pub struct ContentRule {
    pub content: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[allow(non_snake_case)]
pub struct TocRule {
    pub chapterList: Option<String>,
    pub chapterName: Option<String>,
    pub chapterUrl: Option<String>,
    pub isVip: Option<String>,
    pub updateTime: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[allow(non_snake_case)]
pub struct ExploreRule {
    pub bookList: Option<String>,
    pub name: Option<String>,
    pub author: Option<String>,
    pub bookUrl: Option<String>,
    pub coverUrl: Option<String>,
    pub intro: Option<String>,
    pub kind: Option<String>,
    pub lastChapter: Option<String>,
}

// ── Skip/Reason enums ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case, dead_code)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// Non-text source (comic/audio/video) or disabled
    Excluded,
    /// bookSourceUrl is invalid and can't be auto-fixed
    BadUrl,
    /// No searchUrl and no exploreUrl
    NoCapability,
    /// Has searchUrl but missing ruleSearch
    NoSearchRule,
    /// Missing ruleContent (can search but can't read)
    NoContentRule,
    /// Requires login
    LoginRequired,
    /// JS source depends on Legado-specific APIs
    JsApi,
}

// ── Output containers ────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PreflightOutput {
    pub total_input: usize,
    pub excluded: usize,
    pub text_enabled: usize,
    pub eligible: Vec<BookSource>,
    pub skipped: Vec<(BookSource, SkipReason)>,
    pub explore_only: Vec<BookSource>,
    /// Breakdown of eligible sources by searchUrl type
    pub breakdown: PreflightBreakdown,
}

#[derive(Debug, Serialize)]
pub struct PreflightBreakdown {
    pub template: usize,
    pub js_prefix: usize,
    pub js_block: usize,
    pub pure_url: usize,
    pub placeholder: usize,
}
