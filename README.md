# Legado 书源筛选工具

从 `data/b778fe6b.json`（3911 个书源）中自动筛选**当前可用的文字书源**。

## 使用方法

```bash
# 1. 静态预检（无需网络）
cargo run -- preflight

# 2. 搜索测试（50 并发，约 30-45 分钟）
cargo run -- test

# 3. 快速测试（前 10 个）
cargo run -- test --limit 10 --concurrency 5
```

### 常用参数

| 参数 | 说明 |
|------|------|
| `--concurrency N` | 并发数，默认 50 |
| `--timeout N` | 单请求超时（秒），默认 15 |
| `--force` | 忽略缓存，全部重新测试 |
| `--retry-missed` | 仅重测之前失败的源 |
| `--limit N` | 只测前 N 个源 |
| `--rounds N` | 测试轮数，失败源每轮重试（默认 1） |
| `--no-node` | 跳过 JS 源（无需安装 Node.js） |

## 输出

| 文件 | 说明 |
|------|------|
| `output/eligible.json` | 待测书源（2957 个） |
| `output/filtered.json` | 可用的书源，直接导入 Legado |
| `output/missed.json` | 测试失败 |
| `output/skipped.json` | 预检跳过 |
| `output/explore_only.json` | 仅支持发现的书源 |

## 技术栈

- **语言**: Rust (tokio + reqwest)
- **HTML 解析**: scraper (Servo CSS 引擎)
- **编码探测**: chardetng + encoding_rs
- **JS 执行**: Node.js polyfill（可选依赖）
- **缓存**: rusqlite（支持断点续测）
