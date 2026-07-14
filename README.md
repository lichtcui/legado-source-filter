# Legado 书源筛选工具

自动从 [legado.aoaostar.com](https://legado.aoaostar.com) 获取最新书源列表，通过预检过滤和并发搜索测试，筛选出**当前可用的文字书源**，直接导入 [Legado](https://github.com/gedoor/legado) 阅读 APP。

## 快速开始

```bash
# 一行命令：预检 + 5 轮重试测试（全自动，约 5-15 分钟）
cargo run -- full --rounds 5

# 查看当前管道进度
cargo run -- status
```

## 命令详解

### `full` — 一键完成

从下载书源 → 预检 → 多轮测试 → 输出结果，一站式执行。推荐使用。

```bash
# 基本用法
cargo run -- full --rounds 5

# 快速测试前 10 个源
cargo run -- full --limit 10 --rounds 3
```

### `status` — 查看进度

显示预检和测试的当前完成情况。

```bash
cargo run -- status
```

## 全部参数

| 参数 | 适用命令 | 说明 |
|------|---------|------|
| `-c`, `--concurrency N` | full | 并发数，默认 50 |
| `-t`, `--timeout N` | full | 单请求超时（秒），默认 15 |
| `--rounds N` | full | 测试轮数，失败源每轮重试，默认 1 |
| `--limit N` | full | 只测前 N 个源 |
| `--force` | full | 忽略缓存，全部重新测试 |
| `--config PATH` | full | 指定 config.toml 路径（可选） |
| `--json` | 全局 | 输出 JSON Lines 到 stdout，适合 AI 解析 |
| `-i`, `--input PATH` | 全局 | 书源 JSON 路径（默认 XDG_DATA_HOME） |
| `-o`, `--output DIR` | 全局 | 输出目录（默认 XDG_CACHE_HOME） |

## 输出文件

输出目录默认位于 `~/.cache/legado-source-filter/`（可通过 `--output` 或 `XDG_CACHE_HOME` 环境变量修改）。

| 文件 | 说明 |
|------|------|
| `eligible.json` | 预检通过、待测试的书源列表 |
| `filtered.json` | 测试通过的可用书源，直接导入 Legado |
| `missed.json` | 测试失败的书源 |
| `js_api.json` | 依赖 Legado 特有 JS API、无法外部测试的书源 |
| `skipped.json` | 预检阶段跳过的书源及原因 |
| `explore_only.json` | 仅支持「发现」功能、不支持搜索的书源 |
| `report.json` | 结构化汇总报告（JSON 模式） |
| `report.txt` | 人类可读的预检报告 |
| `test_cache.db` | SQLite 缓存，支持断点续测 |

## 技术栈

- **语言**: Rust（tokio + reqwest）
- **HTML 解析**: scraper（Servo CSS 引擎）
- **编码探测**: chardetng + encoding_rs
- **JS 执行**: Node.js polyfill（可选依赖）
- **缓存**: rusqlite（SQLite，支持断点续测）
