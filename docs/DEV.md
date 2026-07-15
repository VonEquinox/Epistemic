# Epistemic — 开发文档

> 配套 docs/PRD.md · v0.2 · 2026-07-15 · 随讨论滚动更新；本文档只描述当前设计

---

## 1. 仓库结构

```
epistemic/
├── docs/                 # PRD.md、DEV.md
├── server/               # Rust cargo workspace
│   ├── crates/core/      # 领域类型、DB 访问（sqlx）、repository 函数
│   ├── crates/api/       # axum HTTP API（bin）
│   ├── crates/worker/    # 后台任务执行器（bin）
│   └── crates/llm/       # Claude API 客户端、prompts、JSON schemas
├── web/                  # Vite + React + TypeScript SPA
├── deploy/               # docker-compose.yml、Caddyfile、备份脚本
└── testset/              # 20—30 篇标注测试集（金标 JSON）
```

## 2. 技术栈

| 层 | 选型 |
|---|---|
| 后端 | Rust stable · axum · sqlx（Postgres，编译期校验 SQL）· tokio · reqwest · serde · utoipa（OpenAPI 导出）· argon2 · tower-sessions |
| 前端 | Vite · React · TypeScript · TanStack Query · zustand · Tailwind · Cytoscape.js（图渲染）· PDF.js（阅读器） |
| API 契约 | utoipa 生成 openapi.json → openapi-typescript 生成前端类型化客户端 |
| 数据 | PostgreSQL 16 + pgvector；全文检索用 Postgres FTS |
| 任务队列 | Postgres jobs 表 + `FOR UPDATE SKIP LOCKED`，worker 为独立二进制 |
| PDF 解析 | GROBID（docker 容器，HTTP 调用，保留 teiCoords 坐标） |
| LLM | Claude API，reqwest 自封装薄客户端（无官方 Rust SDK，走原生 HTTP） |
| 部署 | docker-compose 单机：caddy（自动 HTTPS）+ api + worker + postgres + grobid |

## 3. 系统架构

```
浏览器 SPA（地图/Ego/卡片/PDF/审核队列）
   │ HTTPS
Caddy ──▶ api（axum）──▶ PostgreSQL（唯一事实源：实体/关系/证据/向量/jobs）
                │  ▲
        写 jobs │  │ 读写实体
                ▼  │
             worker（轮询 jobs 表）
                ├──▶ GROBID（PDF → TEI）
                ├──▶ Claude API（DNA 抽取 / 引文分类 / 成对判定）
                ├──▶ Semantic Scholar / arXiv API（元数据、参考文献）
                └──▶ Embedding API（候选召回 + 主题引力，供应商未决）
```

## 4. 数据库 Schema 草案

字段从简，最终以 migration 为准。

```sql
-- 账号与组织
users            (id, email UNIQUE, name, password_hash, role/*admin|member*/, created_at)
invites          (id, email, token, created_by, used_at)
projects         (id, name, description)

-- 论文
works            (id, title_norm, primary_version_id, created_by, created_at)
versions         (id, work_id, kind/*arxiv|conference|journal|preprint|other*/,
                  arxiv_id, doi, url, title, abstract, year, venue_name,
                  pdf_path, tei_path, metadata_source, created_at)
authors          (id, full_name, s2_author_id)
version_authors  (version_id, author_id, position)
work_projects    (work_id, project_id)
citations        (id, citing_work_id, cited_work_id NULL, cited_external JSONB)
merge_history    (id, kept_work_id, merged_work_id, snapshot JSONB, merged_by,
                  created_at, reverted_at)   -- 家族合并可逆

-- DNA 实体（均带 source / review_status / created_by / model_version）
claims           (id, work_id, text, ...)
methods          (id, work_id NULL, parent_id NULL, name, description, ...)
datasets         (id, name, aliases TEXT[])
extractions      (id, version_id, model, prompt_version, raw JSONB,
                  status, usage JSONB, cost_usd, created_at)  -- 原始抽取记录

-- 关系（具体化）
relations        (id, type, aspect, scope, explanation, confidence,
                  source, review_status, created_by_user NULL,
                  model_version NULL, created_at)
relation_members (relation_id, entity_kind/*work|claim|method|dataset*/,
                  entity_id, role/*source|target|input|output*/,
                  anchor_work_id, position)
evidence_spans   (id, relation_id NULL, claim_id NULL, extraction_field NULL,
                  version_id, page, text, bbox JSONB)

-- 协作
reviews          (id, subject_kind/*relation|claim_judgment*/, subject_id,
                  user_id, verdict/*agree|disagree*/, comment, created_at,
                  UNIQUE(subject_kind, subject_id, user_id))
claim_judgments  (id, claim_id, user_id, verdict/*6 档*/, conditions,
                  evidence_url, created_at)
reading_status   (user_id, work_id, status/*5 档*/, starred, updated_at,
                  PRIMARY KEY(user_id, work_id))
annotations      (id, work_id, version_id NULL, user_id,
                  kind/*note|conjecture|question*/, visibility/*private|team*/,
                  anchor JSONB, body, parent_id NULL, resolved, created_at)

-- 距离引擎与向量
embeddings       (entity_kind, entity_id, field, model, vec vector(D),
                  PRIMARY KEY(entity_kind, entity_id, field))
neighbors        (dimension/*citation_coupling|method_lineage|topic*/,
                  work_id, neighbor_work_id, score,
                  PRIMARY KEY(dimension, work_id, neighbor_work_id))
saved_views      (id, name, weights JSONB, created_by)   -- 命名"视角"

-- 任务
jobs             (id, kind, payload JSONB, status/*queued|running|done|failed*/,
                  attempts, run_after, locked_by, locked_at, last_error, created_at)
import_batches   (id, created_by, raw_input TEXT, parsed JSONB, status, created_at)
```

关系状态变更（确认 / 拒绝 / 争议）统一走 core crate 的 repository 函数，函数内负责联动写 jobs（触发 method_lineage 增量更新），避免遗漏。

## 5. API 草案

前缀 `/api/v1`，全部 JSON，session cookie 鉴权。

| 组 | 端点 |
|---|---|
| 认证 | POST /auth/login · /auth/logout · POST /invites（admin）· POST /auth/register?token= |
| 导入 | POST /imports（raw_text → 解析预览）· POST /imports/{id}/confirm · POST /works/quick-add（arXiv/DOI） |
| 论文 | GET /works?query&project&status… · GET /works/{id}（卡片聚合）· PATCH /works/{id} · POST /works/{id}/merge · POST /works/{id}/split |
| PDF | GET /versions/{id}/pdf（鉴权流式）· POST /versions/{id}/pdf（手动上传） |
| 图 | GET /graph/map（全部节点 + 各维 top-32 邻居表 + overlay 数据）· GET /graph/ego/{kind}/{id}?depth&mode |
| 关系 | POST /relations（手动创建）· PATCH /relations/{id}（改类型/方向）· POST /relations/{id}/review（agree/disagree）· GET /review-queue?work&batch |
| 判断 | POST /claims/{id}/judgments |
| 协作 | PUT /works/{id}/reading-status · POST /annotations · GET /works/{id}/annotations |
| 项目 | GET/POST /projects · GET /projects/{id}/coverage（团队覆盖统计） |
| 视角 | GET/POST /views（距离权重组合） |

`GET /graph/map` 返回邻居表而非坐标——布局在前端算（§6.4），权重滑杆不回服务器。

## 6. 前端架构

### 6.1 路由与页面

| 路由 | 页面 |
|---|---|
| /login · /invite/:token | 登录、受邀注册 |
| /map | 全局语义地图（应用首页；图入口策略见 PRD 未决 6） |
| /papers | 论文列表（虚拟化表格 + 筛选） |
| /papers/:id | 论文卡片全页（含 PDF 阅读器） |
| /ego/:kind/:id | Ego 聚焦视图（kind ∈ work / claim / method / dataset） |
| /review | 审核队列 |
| /projects/:id | 项目页（团队覆盖统计） |
| /settings | 管理（邀请、成员） |

地图 / 列表中点开论文 = 侧栏抽屉；直达 URL = 全页。两者复用同一卡片组件。

### 6.2 目录结构

```
web/src/
├── api/          # openapi-typescript 生成的客户端 + TanStack Query hooks
├── stores/       # zustand：距离权重、当前视角、选中态、LOD 档位、面板开合
├── graph/        # 图核心：弹簧长度计算、LOD 控制、样式表、增量布局（纯函数，可单测）
├── pdf/          # PDF.js 封装：高亮层、证据跳转、坐标换算
├── pages/        # 路由页面
└── components/   # RelationBadge、EvidenceQuote、StatusDot、UserAvatarStack 等
```

### 6.3 状态与数据层

- 服务端状态全走 TanStack Query：query key 按资源约定（如 `['work', id]`）；审核裁决、阅读状态用乐观更新，失败回滚并提示
- UI 状态走 zustand，不放服务端数据
- MVP 无 WebSocket：论文卡片的管线进度用 5s 轮询（refetchInterval），其余不需要实时

### 6.4 图渲染

**引擎与布局**

- Cytoscape.js + fcose 布局（支持逐边理想长度）
- 弹簧长度 = Lmin + (Lmax − Lmin) × (1 − score)，score 为各维分数按当前权重的合成值
- 确定性：首次布局以 work_id 哈希生成初始坐标再跑 fcose；结果存 localStorage；此后 `randomize: false`、以既有坐标为初值增量布局；新节点初始放在其最高分邻居旁
- 未接入区：主维度无邻居的节点 `locked` 在画布右缘停靠道（按导入时间排），不参与物理模拟；开启主题引力后解锁并接入 topic 维弹簧

**LOD 三级**（缩放阈值 z1、z2 待调优）

| 档位 | 渲染 |
|---|---|
| 远景 z < z1 | 只有节点圆点，无标签无边 |
| 中景 z1 ≤ z < z2 | 浮现标题标签 |
| 近景 z ≥ z2 | 绘制断言边 + 数量角标，只为视口内节点挂边 |

性能：`textureOnViewport`、`hideEdgesOnViewport`，改动一律包在 `cy.batch` 里。

**边与节点样式**（数据驱动，selector 匹配 data 字段；token 集中在 `graph/styles.ts`，卡片、队列、图例复用）

| data | 样式 |
|---|---|
| status = candidate | 灰虚线 |
| status = confirmed，reviews = 1 | 浅色实线 |
| status = confirmed，reviews ≥ 2 | 深色实线 |
| status = disputed | 红色实线 |
| type = cites | 默认隐藏 |

- 断言束：每（论文对 × 语义组）一条视觉边，`curve-style: bezier` 并行最多 3 条，label 带数量角标；点击边 → 证据面板列出束内全部断言
- 团队 overlay（可开关）：边框粗细 = 组内已读人数；灰色填充 = 无人读；右上红点（SVG background-image）= 未决争议
- Ego 视图：独立 Cytoscape 实例，元素来自 /graph/ego；版本家族用复合节点，双击展开；溢出语义组节点点击原位展开

**数据流**：GET /graph/map → store（节点 + 各维邻居表）→ 滑杆变更 → 前端重算弹簧长度 → 增量 `layout.run()`。全程不回服务器。

### 6.5 PDF 阅读器与证据跳转

- PDF.js 渲染；证据 = {page, bbox}（GROBID teiCoords，PDF 点坐标），用页面 viewport transform 换算后在高亮层画矩形
- 点击卡片字段 / 图边证据 → 滚动到对应页 + 闪烁高亮
- 批注锚点同时存文字引文 + bbox（bbox 失效时按文字回退定位）
- 划选文字弹出批注气泡：选类型（笔记 / 猜想 / 问题）+ 可见性；一键"升格为 Claim / 关系证据"

### 6.6 审核队列交互

全键盘：j / k 上下 · a 接受 · r 拒绝 · e 改类型 · f 调转方向 · u 撤销上一个 · Enter 展开证据 · Esc 关闭面板。裁决即时乐观更新，后台提交。

### 6.7 前端约定

- Tailwind；中文界面；不做暗色模式
- `graph/`、`pdf/` 下的纯函数（弹簧计算、LOD 判档、邻居合成、坐标换算）写单测；组件不强制单测；Playwright 冒烟后置

## 7. 后台任务管线

每篇论文导入后的任务链（每步一行 jobs 记录，失败指数退避重试，≤3 次）：

```
resolve_metadata → fetch_pdf(仅 arXiv 自动) → grobid_parse → extract_dna
      │                                                        │
      └─▶ fetch_references ─▶ update_neighbors(citation_coupling)
                                                               ├─▶ classify_citation_contexts
                                                               ├─▶ embed
                                                               └─▶ propose_pairs(向量召回 K≈10 → 成对判定)
事件触发：关系 确认/拒绝/新候选 ─▶ update_neighbors(method_lineage, 增量 BFS ≤4 跳)
```

worker 单进程多并发（tokio），按 kind 限流：LLM 类任务并发 ≤4，元数据 API 遵守对方限速。论文卡片显示管线进度（哪步完成、哪步失败可重跑）。

## 8. LLM 集成

### 8.1 模型与定价（2026-06 数据，动工时以官方为准）

| 档位 | 模型 ID | 输入 / 输出（$ / MTok） |
|---|---|---|
| 默认（质量优先） | `claude-opus-4-8` | 5 / 25 |
| 中间档 | `claude-sonnet-5` | 3 / 15（2026-08-31 前优惠 2 / 10） |
| 低成本档 | `claude-haiku-4-5` | 1 / 5 |

档位策略（未决 §14）：起步全用 `claude-opus-4-8` 跑测集建质量基线，再用 haiku / sonnet 对比人工接受率，用数据定档。

### 8.2 强制 JSON（结构化输出）

用 `output_config.format`（json_schema 类型）约束输出，API 保证返回合法 JSON：

```json
{
  "model": "claude-opus-4-8",
  "max_tokens": 8000,
  "output_config": { "format": { "type": "json_schema", "schema": { ... } } },
  "messages": [ ... ]
}
```

- schema 中所有 object 必须 `additionalProperties: false` 且列 `required`
- 不支持 minLength / maximum 等约束——应用层二次校验（证据 span 非空、页码存在等），校验失败按"无证据"丢弃字段

### 8.3 Batch API（批量导入用，五折）

批量导入的 DNA 抽取和引文分类走 `/v1/messages/batches`：**token 费用 50%**，多数批次 1 小时内完成。`custom_id` = job id，轮询 `processing_status == "ended"` 后按 custom_id 回填。单批上限 10 万请求。交互式场景（单篇快速添加）走同步接口。

### 8.4 Prompt 缓存

抽取任务共享一个长而稳定的 system prompt（任务说明 + schema + 示例）。做成**固定前缀 ≥ 4096 tokens**（`claude-opus-4-8` 的最小可缓存长度，不足会静默不缓存），尾部加 `cache_control: {"type": "ephemeral"}`：缓存写 1.25×、读 0.1×。易失内容（论文正文）一律放在缓存断点之后。

### 8.5 Rust 客户端约定

- reqwest 薄封装；请求头：`x-api-key`、`anthropic-version: 2023-06-01`、`content-type: application/json`
- **`claude-opus-4-8` 不接受 `temperature` / `top_p` / `top_k`（发了直接 400）**——不写这些字段；深度用 `output_config.effort` 控制（抽取任务 `low` 或 `medium`）
- 429/5xx 指数退避重试并读 `retry-after` 头；4xx 不重试
- 每次调用把 model、prompt_version、usage、成本记入 extractions / jobs
- prompt 模板与 schema 放 `crates/llm/prompts/`，版本号入库，改 prompt 必须升版本

### 8.6 成本量级（估算，以测集实测为准）

按每篇：DNA ~12k 入 / 3k 出，引文分类 ~35 处 × (300 入 / 60 出)，成对判定 K=10 × (3k 入 / 300 出)，合计约 **52k 入 / 8k 出**：

| 方案 | 每篇（Batch 五折后） | 1,000 篇 |
|---|---|---|
| 全 `claude-opus-4-8` | ≈ $0.23 | ≈ $230 |
| 全 `claude-haiku-4-5` | ≈ $0.05 | ≈ $46 |

一次性成本（每篇只抽一次并缓存）；prompt 缓存会进一步压低。

### 8.7 Embedding

Claude API 无 embedding 端点，需外部方案（未决 §14）：候选为 Voyage AI 一类托管 API，或本地小模型 sidecar。选择决定 pgvector 维度 D。用途：候选召回与主题引力，同一套向量复用。

## 9. 距离引擎（服务端计算）

**citation_coupling**：纯 SQL 集合运算。bibliographic coupling = |共同参考文献| / √(|R_a|·|R_b|)，co-citation 同理取共同施引方；两者取较大值。新论文导入时算其 top-32 并反向更新受影响论文。

**method_lineage**：边集 = relations 中方法组类型（uses_method_from / improves_on / alternative_to）且 review_status ≠ rejected；边长 confirmed = 1、候选 = 2；从状态变更节点增量 BFS（≤4 跳），score = 1/d，写回 neighbors。

**topic（主题引力，默认关闭）**：embeddings 表 pgvector 余弦 top-32。

前端消费方式见 §6.4。

## 10. 认证与安全

- 注册邀请制（admin 生成邀请链接）；argon2id 存密码；session cookie（HttpOnly + Secure），tower-sessions 存 Postgres
- PDF 只经鉴权端点流式返回，磁盘目录不对外；robots.txt 全站禁抓
- Caddy 自动 HTTPS；登录接口限速
- 每日 pg_dump + PDF 目录增量备份到服务器外（cron + rclone/rsync）
- 密钥只存服务器 .env，仓库只放 .env.example

## 11. 三人分工与协作契约

| 线 | 范围 |
|---|---|
| A 后端内核 | schema/migrations、认证邀请、works/versions/projects CRUD、清单解析与去重家族、图查询（map/ego 聚合）、OpenAPI 导出 |
| B 前端 | §6 全部：地图 + Ego、卡片、PDF 阅读器、审核队列、列表、overlay |
| C AI 管线 + 距离引擎 | worker 框架、GROBID 接入、prompts 与 schemas、Batch 编排、引文分类、embedding、距离维度、测集与评估脚本 |

契约：A 产出 openapi.json → B 生成类型化客户端；A 先交付 **10 篇样例论文的 fixture 数据**（含 DNA、关系、证据），B 不等 C 的真实管线即可开发全部 UI；C 通过 core crate 的 repository 函数写库，不直接拼 SQL。

## 12. 里程碑任务分解

| 阶段 | A 后端 | B 前端 | C AI 管线 |
|---|---|---|---|
| M0 | schema SQL 草稿、repo 骨架、docker-compose | 原型图（地图/卡片/队列线框） | 标注 20—30 篇测集、写金标 JSON |
| M1 | 认证邀请、导入解析、元数据、去重家族、列表 API、fixtures | 列表、卡片（吃 fixture）、阅读状态 UI | worker 框架、GROBID 跑通、元数据任务 |
| M2 | 证据/claim 存储、PDF 端点 | PDF 阅读器、高亮、证据跳转 | DNA 抽取 + 结构化输出 + 过测集门槛 |
| M3 | 图查询 API、neighbors 表、审核 API | 全局地图 + Ego + 审核队列 + overlay | 引文分类、成对判定、两维距离、Batch 编排 |
| M4 | 修 bug、备份上线 | 打磨交互 | 接受率统计、prompt 迭代 |

## 13. 开发约定

- `cargo fmt` + `clippy -D warnings`；前端 `tsc --noEmit` + eslint——提交前必过
- migration 用 `sqlx migrate`，只增不改已合并的 migration
- main 分支 + 短生命 feature 分支，PR 需另一人过目（AI 生成的代码更要看）
- 测试策略：家族匹配 / 去重 / 距离计算 / 前端图纯函数写单测；AI 质量看测集指标（PRD §6.5）；E2E 冒烟后置
- commit 信息说清"为什么"，backlog 记在 GitHub Issues

## 14. 未决

1. Embedding 供应商（Voyage API vs 本地 sidecar）→ 定 pgvector 维度
2. LLM 档位策略（先全 opus-4-8 建基线，测集数据出来后定）
3. 论文清单实际格式（同 PRD §11.2）→ 决定 /imports 解析器
