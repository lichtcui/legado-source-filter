# CLAUDE.md

本文件为 Claude Code 提供此项目的上下文指引。

## 构建与测试

```bash
cargo build                              # 编译
cargo test                               # 全部测试（33 个）
cargo test --test integration            # 集成测试
cargo run -- full --rounds 5             # 一键下载+预检+5轮测试（~5-15 分钟）
cargo run -- full --limit 10 --rounds 3  # 快速测试前 10 个
cargo run -- --json status               # 查看管道进度（JSON 格式）
cargo run -- status                      # 查看管道进度（人类可读）
cargo run -- --json full --limit 5       # 一键运行+JSON 输出（AI 友好）
```

## 管道流程

0. `自动更新` — 从 legado.aoaostar.com 获取最新全量书源 JSON（本地缓存，增量更新）
1. `preflight` — ~3911 个源 → ~2957 个文字+启用源。排除非文字/禁用、URL 无效、无搜索能力的源。
2. `test` — 每源最多试 3 个关键词（checkKeyWord → 通用关键词 → config.toml 书）。50 并发请求。结果缓存在 SQLite 中，支持断点续测。

## 自动续跑

全量测试（2957 源 × 5 轮）约需 5-15 分钟，第 1 轮为主，后 4 轮只重试极少量 network_error 源。SQLite 缓存支持断点续测：

```bash
# 首次启动（自动清除旧缓存，从头跑）
cargo run -- full --rounds 5

# 如果超时或中断：查进度，然后续跑
cargo run -- --json status              # 看已完成/剩余多少
cargo run -- full --rounds 5            # 续跑——已有缓存的源直接跳过
```

关键原则：
- `full` 命令**默认清除旧缓存**，确保从头重新测试。续跑直接重新执行 `full` 命令即可，已有缓存的源自动跳过
- SQLite 缓存（`output/test_cache.db`）保存每源最终状态，重复执行只测未完成项
- `--rounds N` 的后几轮只重试 `network_error` 源，不重试 `passed` / `dead_domain` / `no_results`
- 用 `--json` flag 输出结构化结果，方便 AI 解析进度和状态

## 架构

```
src/
├── main.rs          — CLI 入口（clap，full / status 子命令）
├── lib.rs           — 库导出，供集成测试使用
├── types.rs         — BookSource、规则结构体、枚举
├── preflight.rs     — 5 步预检分类管道
├── url_fixer.rs     — 自动修复残缺 bookSourceUrl
├── search_url.rs    — 从 searchUrl 构造 HTTP 请求（{{key}}、POST、JS L1）
├── http_client.rs   — reqwest 封装（chardetng 编码探测 + encoding_rs 转码）
├── tester.rs        — 异步测试调度（tokio JoinSet + Semaphore）
├── reporter.rs      — JSON 报告输出
├── db.rs            — SQLite 缓存层（断点续测）
├── rule_dsl/        — Legado 自定义规则 DSL 解析器
│   ├── parser.rs         — 词法分析器
│   ├── html_extractor.rs — HTML 规则求值（scraper）
│   ├── json_extractor.rs — JSONPath 求值
│   └── combinator.rs     — || 回退、## 正则替换、!0 索引
└── js_polyfill/     — JS searchUrl 处理引擎
    ├── polyfill.js       — 模拟 java.ajax/post/put/get/cookie/source
    └── runner.rs         — node --eval 子进程管理
```

## 关键数据

- 输入：从 legado.aoaostar.com 自动获取最新全量书源（~3911 个，~21MB），本地 `data/` 目录缓存
- 文字+启用：3680 → 预检 → 2957 个待测（683 跳过、46 仅发现）
- searchUrl 类型：2785 模板 + 129 @js + 21 `<js>` + 22 纯 URL
- 输出：`output/filtered.json`（约 640 个可用源，导入 Legado）

## CLI 参数

| 参数 | 说明 |
|------|------|
| `full` | 一键 preflight + test（推荐） |
| `status` | 查看当前管道进度（支持 `--json`） |
| `--json` | 全局 flag，输出 JSON Lines 到 stdout（AI 友好） |
| `--force` | 忽略缓存，全部重新测试 |
| `--limit N` | 只测前 N 个源 |
| `--rounds N` | 测试轮数，失败源每轮重试（默认 1） |
| `--concurrency N` | 并发数，默认 50 |

## Legado DSL

规则字符串是以 `@` 分隔的管道：`class.item@tag.li.0@text` 表示 "查找 .item，在其中找 li，取第 1 个，提取文本"。详见 `rule_dsl/parser.rs:RuleToken`。
