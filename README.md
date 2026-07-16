# Epistemic

研究组内部的论文证据关系图与集体研究记忆工具。

- 需求：[`docs/PRD.md`](docs/PRD.md)
- 开发：[`docs/DEV.md`](docs/DEV.md)

## 技术栈

| 层 | 选型 |
|---|---|
| 前端 | Vite · React · TypeScript · Cytoscape.js · PDF.js |
| 后端 | Rust · axum · sqlx · tokio |
| 数据 | PostgreSQL 16 + pgvector |
| 解析 | 无 GROBID（PDF 存盘；DNA 暂用标题/摘要 + LLM；可后续接 VLM） |
| LLM | OpenAI Chat Completions（兼容网关） |
| 部署 | docker-compose + Caddy |

## 快速开始

```bash
# 1. 复制环境变量
cp .env.example .env

# 2. 启动依赖（Postgres）
cd deploy && docker compose up -d postgres

# 3. 后端
cd ../server
cargo run -p epistemic-api     # HTTP API :8080
cargo run -p epistemic-worker  # 后台任务

# 4. 前端
cd ../web
npm install
npm run dev                    # :5173
```

## 仓库结构

```
docs/                 # PRD / DEV
server/               # Rust cargo workspace
  crates/core/        # 领域类型、DB、repository
  crates/api/         # axum HTTP API
  crates/worker/      # 后台任务执行器
  crates/llm/         # Chat Completions 客户端、prompts
web/                  # Vite + React SPA
deploy/               # docker-compose、Caddy、备份
testset/              # 标注测试集
```

## 三人分工

| 线 | 范围 |
|---|---|
| A 后端内核 | schema、认证、CRUD、图查询、OpenAPI |
| B 前端 | 地图 / Ego / 卡片 / PDF / 审核队列 |
| C AI 管线 | worker、prompts、距离引擎、测集（PDF 全文解析待接） |
