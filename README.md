# Epistemic

研究组内部的**论文证据关系图 + 集体研究记忆**工具。给本组用的、每条边都带原文证据的论文关系图，叠加组内阅读状态、判断与讨论。

> 完整文档见 [`docs/PROJECT.md`](docs/PROJECT.md)（定位 + 系统设计 + 数据模型 + API + 部署 + 实现现状）。

## 它解决什么

- 论文之间"到底什么关系"（换了损失函数 / 结论只在小数据成立）只存在个人脑中——引用图说不出来。
- 谁读过什么、读到多深不可见；判断（可信吗？复现过吗？）散落群聊无法沉淀。
- 三层信息严格分级：**公开事实 / 团队记录 / AI 候选**，永不混淆；**无证据不入图**；相似度只决定位置、永不画成边。

## 技术栈

| 层 | 选型 |
|---|---|
| 前端 | Vite · React · TypeScript · Cytoscape.js + fcose · pdfjs-dist · TanStack Query · zustand · Tailwind |
| 后端 | Rust · axum · sqlx · tokio（Cargo workspace：core / api / worker / llm / mcp） |
| 数据 | PostgreSQL 16 + pgvector；任务队列 = jobs 表（`FOR UPDATE SKIP LOCKED`）|
| AI | OpenAI Chat Completions 兼容 API（DNA 分面抽取 / 引文分类 / 成对判定，支持 Batch）+ SiliconFlow Qwen3-Embedding-8B |
| 解析 | 有 PDF：`pdftoppm` 全页转 PNG → VLM；无 PDF：arXiv HTML 全文 |
| 部署 | docker-compose（postgres / api / worker / caddy）+ Caddy 自动 HTTPS |

## 快速开始

```bash
# 1. 环境变量（至少填 OPENAI_API_KEY、EMBEDDING_API_KEY）
cp .env.example .env

# 2. 启动 Postgres（pgvector/pgvector:pg16）
cd deploy && docker compose up -d postgres

# 3. 后端（api 启动时自动跑 migration）
cd ../server
cargo run -p epistemic-api      # HTTP API :8080
cargo run -p epistemic-worker   # 后台任务

# 4. 前端
cd ../web
npm install
npm run dev                     # :5173（开发代理转发 /api → :8080）
```

首次运行：设 `BOOTSTRAP_ADMIN_EMAIL` 等变量可在空库时自动建管理员；`scripts/seed_fixtures.sql` 可灌 11 篇经典论文样例数据。

## 核心概念

- **用户 → 研究组 → 图**：论文库全局共享，图（graph）是组内按成员权限裁剪的地图工作区；同一篇论文在不同图里有各自的讨论流。首页即 `/groups`。
- **多层 DNA（aspects）**：每篇论文 8 个固定分析层（problem / contributions / methods / theory / datasets / findings / limitations / positioning），各自带 embedding，地图默认按选定层的相似度布局。
- **关系是具体化的边**：12 种白名单类型，绑定原文证据 span，`review_status` 由成员 agree/disagree 推导（有赞有反 = 争议）；高风险类型（fails_to_reproduce / contradicts_claim）确认前不上图。
- **审核队列**：AI 提候选，人全键盘一键裁决（j/k/a/r/f/e，20 步撤销）。

## Codex / MCP

内置只读 MCP server（`epistemic-mcp`，stdio + 个人 token 鉴权），把图快照、节点评论、论文原文、关系按用户权限暴露给 Codex / Claude 等编码助手。见 [`docs/PROJECT.md` §9](docs/PROJECT.md)。

## 仓库结构

```
docs/PROJECT.md       # 唯一文档
server/crates/{core,api,worker,llm,mcp}/
web/                  # Vite + React SPA
deploy/               # docker-compose、Caddyfile、backup.sh
scripts/              # seed_fixtures.sql、requeue_aspect_pipeline.sql
testset/              # 金标测试集（11 篇）+ eval_gold.py
```
