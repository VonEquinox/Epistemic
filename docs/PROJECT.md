# Epistemic — 项目文档

> 单一文档：产品定位 + 系统设计 + 实现现状。以**当前代码为准**描述，随实现滚动更新。
> v1.0 · 2026-07-17 · 取代原 PRD.md / DEV.md / MCP.md

---

## 1. 定位

研究组（ML 方向）内部的**论文证据关系图 + 集体研究记忆**工具。3 人建设、全员 AI 辅助开发，首批用户即本组（dogfood），部署在一台公网服务器上，不做商业化。

要解决的问题：

1. 论文之间"到底什么关系"只存在于个人脑中——引用图只说"B 引了 A"，说不出"B 换掉了 A 的损失函数"。
2. 谁读过什么、读到什么深度不可见。
3. 判断（可信吗？复现过吗？）散落在群聊里，无法沉淀。
4. 新成员没有可靠的入门阅读路径。

一句话：**每条边都带原文证据的论文关系图，叠加组内阅读状态、判断与讨论。**

### 产品原则（依然有效）

1. **三层信息严格分级**：`public_fact` / `team_record` / `ai_candidate`，数据层字段区分、视觉层样式区分，永不混淆。
2. **无证据不入图**：AI 抽取的字段和关系必须绑定原文 span（页码 + 文字），点击跳 PDF；给不出证据的丢弃。
3. **判断落在 Claim / 方法组件级**，不给整篇论文打分。
4. **零确认也有用**：纯自动层（元数据 + 引用 + DNA）自身成立，人工确认是增益。
5. **相似度只决定位置，永不画成边**；画出来的边只有断言关系（有证据、可审核）。
6. 所有编辑可追溯、可撤销、可争议；`ai_candidate` 不经人工确认永不升级为 `confirmed`。
7. 高风险类型（`fails_to_reproduce` / `contradicts_claim`）确认前不上图。
8. 录入靠 AI 提议 + 人一键裁决，不靠人手填表单。

### 不做

公共平台、全球论文库、整篇论文对错标签、Neo4j / 微服务 / K8s / Redis / 独立向量库、通知系统、移动端、暗色模式。

---

## 2. 系统架构

```
浏览器 SPA（研究组/地图/Ego/卡片/PDF/审核队列）
   │ HTTPS (Caddy)
   ├──▶ api（axum, :8080）──▶ PostgreSQL 16 + pgvector（唯一事实源）
   │        │ 写 jobs            实体/关系/证据/评论/向量/jobs/会话
   │        ▼
   │     worker（轮询 jobs 表，FOR UPDATE SKIP LOCKED）
   │        ├──▶ Chat Completions API（DNA 分面抽取 / 引文分类 / 成对判定，含 Batch）
   │        ├──▶ arXiv API + arXiv HTML（元数据 / 全文）
   │        └──▶ Embedding API（SiliconFlow Qwen3-Embedding-8B, dim 4096）
   │
   └─(本机 stdio)─▶ epistemic-mcp（只读 MCP server，个人 token 鉴权，供 Codex/Claude 等）
```

| 层 | 选型 |
|---|---|
| 前端 | Vite 6 · React 18 · TypeScript · TanStack Query v5 · zustand · Tailwind 3 · Cytoscape.js + fcose · pdfjs-dist 4 |
| 后端 | Rust stable · axum · sqlx（编译期校验 SQL）· tokio · reqwest · argon2 · tower-sessions（Postgres 存储） |
| 数据 | 单 PostgreSQL 16 + pgvector；任务队列 = jobs 表 |
| 解析 | **有 PDF 时**：`pdftoppm` 全页转 PNG → VLM 多模态抽取；**无 PDF 时**：arXiv HTML 全文 → 文本抽取；兜底 title+abstract |
| LLM | OpenAI Chat Completions 兼容 API（自封装薄客户端），支持 Batch |
| MCP | `rmcp` stdio server，个人 token（`epm_…`，SHA-256 存哈希） |
| 部署 | docker-compose（postgres / api / worker / caddy）+ Caddy 自动 HTTPS |

Cargo workspace 五个 crate：`core`（领域类型 + migrations + repository）、`api`、`worker`、`llm`、`mcp`。仓库结构：

```
docs/PROJECT.md       # 本文档
server/crates/{core,api,worker,llm,mcp}/
web/                  # Vite + React SPA
deploy/               # docker-compose、Caddyfile、backup.sh
scripts/              # seed_fixtures.sql、requeue_aspect_pipeline.sql
testset/              # 金标测试集 + eval_gold.py
```

---

## 3. 数据模型（以 migrations 001–007 为准）

### 3.1 工作区层级：用户 → 研究组 → 图

这是比初版设计新增的一层（migration 004），也是**当前的主导航**：

- `research_groups`：研究组；`group_members`（PK group_id+user_id，`role ∈ owner|admin|member`）。
- `graphs`：图工作区，属于且仅属于一个组；建组时自动创建一张"主图"。
- `graph_works`：论文挂进图的成员表。**论文库（works）全局共享，图是其上按成员权限裁剪的视图**。
- 所有组/图接口经 `require_member` / `require_graph_access` 校验成员身份。

旧的 `projects` / `work_projects`（扁平标签 + 团队覆盖统计）仍然保留并可用，与组/图并存。

### 3.2 论文与去重

- `works`（家族/一项工作）→ `versions`（arxiv/conference/journal/preprint/other，含 `arxiv_id`/`doi` 部分唯一索引、`pdf_path`/`tei_path`）→ `authors` / `version_authors`。
- `citations`：`cited_work_id` 可空 + `cited_external JSONB` 存库外引用。
- `merge_history`：合并**可逆**——JSONB snapshot 记录版本、项目、图成员、aspects、阅读状态、关系成员、邻居，支持 split 完整还原（`works.primary_version_id` 外键 DEFERRABLE 即为此设计）。

### 3.3 DNA 与多层分析（aspects）

- `claims` / `methods`（parent 自引用层级）/ `datasets`；`extractions` 记录每次抽取的 model、prompt_version、raw JSONB、usage、cost、job_id（唯一，幂等）。
- **`paper_aspects`**（migration 003）：每篇论文 8 个固定分析层，PK (work_id, aspect)，字段 summary / bullets / source_text / page / model / prompt_version。8 个 aspect key：

  `problem · contributions · methods · theory · datasets · findings · limitations · positioning`

  每个 aspect 对应一个邻居维度 `aspect_{key}` 和一个 embedding field `aspect:{key}`——这就是"多层 DNA + 分面 embedding + 按层相似连图"。

### 3.4 关系（具体化边）

- `relations`：`type`（12 种白名单枚举：cites, version_of, uses_method_from, improves_on, alternative_to, uses_dataset_from, compares_against, reproduces, fails_to_reproduce, supports_claim, contradicts_claim, prerequisite_for）+ aspect / scope / explanation / confidence / source / review_status / model_version。
- `relation_members`：多态成员（entity_kind ∈ work|claim|method|dataset|version|person，role ∈ source|target|input|output），`anchor_work_id` 把任意粒度投影为 Work 层的边。当前校验强制**恰好一个 source + 一个 target**（数据模型天然支持超边，UI 未开放）。
- `evidence_spans`：绑定 relation / claim / extraction_field 三者至少其一；version_id + page + text + bbox JSONB。

### 3.5 协作

- `reviews`：对 relation 或 claim_judgment 的 agree/disagree，每人每对象一条。**`review_status` 是从 reviews 推导的**：有赞有反 → `disputed`；只反 → `rejected`；只赞 → `confirmed`；无 → `unreviewed`。
- `claim_judgments`：6 档（supported / partially_supported / contradicted / not_reproduced / concern / unclear）+ conditions + evidence_url；判断本身可被 review，被争议时连带标记 claim disputed。
- `reading_status`：5 档（unread / skimmed / read / reproduced / needs_review）+ starred。
- **两套评论系统并存**：
  - `annotations`（work 级、PDF 锚定）：kind ∈ note|conjecture|question，visibility ∈ private|team，anchor JSONB（page+text+bbox），parent_id 线程。
  - `node_comments`（migration 006，**graph_id × work_id 作用域**）：kind ∈ comment|idea|thinking|review|question|critique，visibility private|team，线程回复。同一篇论文在不同图里有各自独立的讨论流。

### 3.6 距离引擎与向量

- `embeddings`：PK (entity_kind, entity_id, field)，`vec vector(4096)`（Qwen3-Embedding-8B 原生维度，migration 002）。
- `neighbors`：PK (dimension, work_id, neighbor_work_id)。维度 = `citation_coupling` / `method_lineage` / `topic` + 8 个 `aspect_*`。
- `saved_views`：命名的权重组合（"视角"）。

### 3.7 任务与账号

- `jobs`：kind + payload + status(queued|running|done|failed) + attempts + run_after + `dedupe_key`（排队中部分唯一，migration 005 加的幂等护栏）。领取用 `FOR UPDATE SKIP LOCKED` + kind 优先级排序 + 2 小时租约回收。
- `import_batches`（+ `processing_started_at` 防重复确认）；`batch_applied_items`：LLM Batch 回填的幂等台账。
- `users`（admin|member）、`invites`（邀请制注册）、`tower_sessions`（会话）。
- `mcp_access_tokens`（migration 007）：个人 MCP token，`epm_` 前缀、只存 SHA-256 哈希、可撤销、记录 last_used_at。

---

## 4. 后台管线（worker）

Job kinds（实际字符串）：`resolve_metadata` · `fetch_pdf` · `extract_dna`（旧 `grobid_parse` 已废弃，自动映射到它）· `fetch_references` · `classify_citation_contexts` · `propose_pairs` · `embed` · `update_neighbors_citation` · `update_neighbors_lineage` · `batch_orch`。

```
quick-add / import 确认
  └─▶ resolve_metadata（arXiv API；限速时回退 arXiv HTML 页）
        └─▶ fetch_pdf（仅 arXiv 自动下载；落盘后）
              └─▶ extract_dna ──┬─▶ embed（8 个 aspect 向量 + 摘要向量）
                                ├─▶ update_neighbors_citation
                                └─▶ propose_pairs
  └─▶ fetch_references / classify_citation_contexts ─▶ propose_pairs

事件触发：关系确认/拒绝/新候选 ─▶ update_neighbors_lineage（增量 BFS ≤4 跳）
```

**extract_dna 的取源优先级**（`jobs/extract.rs`，prompt 版本 `dna_aspects_v1`）：

1. **有 PDF** → `pdftoppm` 全页转 PNG data URLs → VLM 多模态 `complete_json_vision`（页码 = 图片序号，天然可信）；
2. 无 PDF、有已存 HTML → 文本抽取；
3. 无 PDF、有 arxiv_id → 抓 `arxiv.org/html` / `ar5iv` 全文 HTML，存盘后文本抽取（此路径页码可能为 0）；
4. 兜底 title + abstract。

正文截断 120k 字符；输出一次性包含 **8 个 aspect + 参考文献书目**，强约束 JSON schema（`crates/llm/prompts/dna_aspects_v1.md` + `dna_aspects_schema_v1.json`）。抽取结果经 checkpoint（extractions.job_id 唯一）保证重试幂等；写库时整体替换该 version 的生成数据并在同一事务里 enqueue 下游任务（dedupe_key 去重）。

候选关系两条路（PRD 原则不变）：**引文上下文分类为主力**（classify_citation_contexts，便宜、自带证据 span）；**向量召回 + 成对判定为补充**（propose_pairs，覆盖相似但互不引用的对，限额）。置信度 < 0.5 丢弃；0.5–0.75 默认隐藏；> 0.75 进审核队列；高风险类型一律人审。

worker 单进程多并发（`WORKER_CONCURRENCY`，默认 16），失败指数退避重试 ≤3 次。论文卡片显示管线进度，失败任务可从卡片重跑（admin 亦可走 `/jobs/requeue`）。

---

## 5. LLM 与 Embedding

**Chat Completions**（`crates/llm`，类型 `LlmClient`）：

- 协议 `POST {base}/v1/chat/completions`，Bearer 鉴权；兼容 OpenAI 官方 / OpenRouter / vLLM / 中转站。
- 结构化输出用 `response_format.json_schema (strict)`；网关不支持时客户端做宽松 JSON 解析（去 markdown fence）；应用层二次校验，无证据字段丢弃。
- 429/5xx 指数退避并读 retry-after；4xx 不重试。每次调用的 model / prompt_version / usage / 成本入 extractions 与 jobs；`estimate_cost_usd` 仅粗估。
- **Batch**：批量 DNA 走 OpenAI Batch（上传 JSONL → 轮询 → 按 custom_id 回填），`batch_orch` job 编排，`batch_applied_items` 保证回填幂等；不支持 Batch 的网关自动只用同步接口。
- prompt 模板与 schema 在 `crates/llm/prompts/`，版本号入库，改 prompt 必须升版本。

**Embedding**（与 Chat 解耦，OpenAI 兼容 `/embeddings`）：默认 SiliconFlow `Qwen/Qwen3-Embedding-8B`，dim 4096。用途：8 个 aspect 向量 + 候选召回 + topic 邻居。换模型需新 migration 改 `vector(D)` 并全量重嵌。

---

## 6. 距离引擎

服务端预计算每维 top-32 邻居写入 `neighbors` 表；前端拿邻居表本地算布局，调参不回服务器。

| 维度 | 计算 |
|---|---|
| `citation_coupling` | 纯 SQL：bibliographic coupling 与 co-citation 归一化重合度取较大值；导入时增量更新 |
| `method_lineage` | 方法组关系（uses_method_from / improves_on / alternative_to，非 rejected）图距离；confirmed 边长 1、候选 2；增量 BFS ≤4 跳，score = 1/d |
| `topic` | 摘要 embedding 余弦 top-32（旧"主题引力"，默认关闭） |
| `aspect_*` × 8 | 各 aspect embedding 余弦 top-32——**当前地图的默认布局来源** |

---

## 7. HTTP API（axum，前缀 `/api/v1`）

鉴权：session cookie（tower-sessions + Postgres，SameSite=Lax，14 天不活动过期，`SESSION_SECURE=true` 开 Secure）；`AuthUser` / `AdminUser` 提取器。邀请制注册；argon2id 存密码。`GET /health` 无鉴权。

| 组 | 端点 |
|---|---|
| 认证 | POST /auth/login · /auth/logout · GET /auth/me · POST /auth/register（凭邀请 token）· POST /auth/invites（admin）· GET /auth/users |
| MCP token | GET/POST /auth/mcp-tokens · DELETE /auth/mcp-tokens/{id}（明文只在创建时返回一次） |
| 论文 | GET /works?query… · POST /works/quick-add（arXiv/DOI）· GET /works/{id}（卡片聚合：版本/作者/claims/methods/aspects/关系/证据/阅读/管线）· POST /works/{id}/merge · /split · GET /works/{id}/evidence · /claims-full |
| 导入 | POST /imports（raw_text → 预览）· GET /imports/{id} · POST /imports/{id}/confirm（原子认领 + 建 works + 入队管线） |
| 组/图 | GET/POST /groups · GET /groups/{id} · GET/POST /groups/{id}/members · GET/POST /groups/{id}/graphs · GET /groups/graphs/{gid} · POST/DELETE /groups/graphs/{gid}/works[/{work_id}] · POST /groups/graphs/{gid}/import-library |
| 图视图 | GET /graph/map（节点 + 各维邻居表 + 断言边；`?graph_id=` 限定单图）· GET /graph/ego/{kind}/{id}?depth&mode（≤30 节点，溢出聚语义组） |
| 关系 | POST /relations（手动，team_record/confirmed）· GET/PATCH /relations/{id}（改类型/转向/改状态）· POST /relations/{id}/review · GET /review-queue |
| Claim | POST /claims · POST /claims/promote（划选升格）· GET /claims/{id} · GET/POST /claims/{id}/judgments · GET /claims/judgments/{id} · POST /claims/judgments/{id}/review |
| 证据 | POST /evidence · GET /evidence/{id} |
| 协作 | PUT /works/{id}/reading-status · GET/POST /works/{id}/annotations · GET/POST /graphs/{gid}/works/{wid}/comments · PATCH/DELETE /comments/{id} |
| 项目 | GET/POST /projects · GET /projects/{id} · GET /projects/{id}/coverage · POST /projects/{id}/works/{work_id} |
| PDF | GET /versions/{id}/pdf（鉴权流式）· POST /versions/{id}/pdf（multipart 上传 ≤100 MiB，落盘后自动入队 extract_dna）· GET /versions/{id}/evidence |
| 视角 | GET/POST /views · GET/DELETE /views/{id} |
| 任务（admin） | POST /jobs/batch-dna（≤500 versions）· GET /jobs/work/{work_id} · POST /jobs/requeue |

图/地图返回**邻居表而非坐标**——布局在前端算，权重滑杆纯前端重排。

---

## 8. 前端（web/）

### 路由

| 路由 | 页面 | 状态 |
|---|---|---|
| `/groups`（**首页**）· `/groups/:id` | 研究组列表 / 组内多图管理（建图、导入全库、打开地图） | ✅ |
| `/map` | 全局/单图语义地图（`?group=&graph=` 限定作用域） | ✅ |
| `/papers` · `/papers/:id` | 论文列表 + 论文工作台（卡片 + 节点评论 + 批注 + PDF 阅读器） | ✅ |
| `/ego/:kind/:id` | Ego 聚焦视图（深度 1–2、模式、证据面板内直接 agree/disagree） | ✅ |
| `/review` | 审核队列（全键盘 j/k/a/r/f/e/Enter/u，20 步撤销栈） | ✅ |
| `/projects` · `/projects/:id` | 项目 + 团队覆盖统计 | 基础 |
| `/import` | 批量粘贴导入（预览为原始 JSON，待打磨） | 基础 |
| `/login` · `/invite/:token` · `/settings` | 登录 / 受邀注册 / 账号 + MCP token + 邀请管理 | ✅ |

### 地图（MapPage + graph/MapView）

- **默认是 aspect 分层模式**（初始 `methods` 层）：8 个层 chip 切换，每层按对应 `aspect_*` 邻居布局并画相似度参考边；最小相似度滑杆（0.25–0.9）实时过滤不重排。另有"综合布局"旧模式：citation_coupling + method_lineage 权重滑杆混合（可存/读命名视角），不画相似边。
- 双层边架构：稀疏隐形"布局弹簧"（top-4，分数≥0.4）先行驱动 fcose；较密的可见相似边（top-12）布局后再挂，不压垮间距。弹簧长度 160–720px 随分数平方衰减。
- LOD 三级（z1=0.6, z2=1.2）：远景只有圆点 → 中景出标题 → 近景才画断言边；`textureOnViewport` + `hideEdgesOnViewport`。
- 断言边样式：AI 候选灰虚线 / 单人确认浅色 / 多人确认深色 / 争议红色；cites 永不画；高风险类型确认前不显示；未审候选默认隐藏（可开关）。断言按（论文对 × 语义组）捆束，最多并行 3 条，带数量角标。
- 节点编码：已读人数 → 边框粗细；无人读 → 灰填充；有争议 → 红边框；无邻居节点锁定停靠右缘"未接入区"。布局确定性：id 哈希种子位置 + fcose 增量。
- 单击 → 侧栏抽屉（完整卡片 + 节点评论）；双击 → Ego。

### 卡片、PDF 与证据跳转

- PaperCard：基本信息 / 摘要 / **多层分析**（8 aspect，每层 summary + bullets + 原文引文 + 页码）/ claims（6 档判断表单）/ methods / 关系分组列表（前置/改进/被改进/复现/冲突…）/ 管线进度与失败重跑。
- PdfViewer（pdfjs-dist 直用）：鉴权 blob 加载、全页渲染 + 文本层；**证据跳转已实装**——bbox 优先（支持多种 bbox 格式），无 bbox 回退文本搜索定位；平滑滚动 + 琥珀色闪烁高亮；`?page=&evidence=` 可深链。
- 划选文字 → 气泡：建批注（笔记/猜想/问题 × 私人/团队）或一键"升格为 Claim"（POST /claims/promote）。
- 节点评论（NodeComments）：图作用域，6 种类型（评论/Idea/思考/Review/问题/批评），私人/团队，线程回复，可编辑删除。

### 状态与数据层

- TanStack Query v5（约 45 个 hooks）：审核裁决 / 关系编辑 / 阅读状态乐观更新 + 失败回滚；卡片在管线运行中每 5s 轮询（无 WebSocket）。
- zustand 单 store：权重、活动 aspect、断言边开关、相似度阈值、组/图上下文、抽屉选中态、LOD。
- API 客户端为手写 fetch 封装 + 手工维护的 `api/types.ts`（**未接 OpenAPI codegen**）。
- 纯函数（弹簧长度、LOD、邻居合成、bbox 换算）集中在 `graph/` `pdf/`；`npm run test:layout` 跑布局单测。

---

## 9. MCP Server（crates/mcp）

只读、**stdio** 传输（`rmcp`），进程级绑定一个用户：启动时用 `EPISTEMIC_MCP_TOKEN` 换出用户身份，之后所有数据按该用户的组成员关系裁剪——只见自己所在组的图；关系/引用/邻居只返回两端都在所选图内的边；私人评论只对作者本人可见。直连 Postgres（不跑 migration），PDF 文本用系统 `pdftotext` 现场抽取。

| 工具 | 用途 | 关键参数 |
|---|---|---|
| `help` | 用法指南 | `topic?`: all/workflow/comments/source/relations |
| `list_graphs` | 我可见的组和图 | — |
| `get_graph_snapshot` | 图快照：节点 + 邻居 + 断言边 + 可见评论计数（分页） | `graph_id` · `offset` · `limit`（默认 500，≤2000） |
| `get_node_context` | 单节点全景：元数据 + aspects + claims/methods + 证据 + 近 50 条评论 + 关系摘要 | `graph_id` · `work_id` |
| `get_node_comments` | 成员评论增量读取 | `since`(RFC3339) · `kinds` · `limit`（≤1000） |
| `get_node_source` | 论文原文：不指定页 → HTML/TEI 优先；指定页 → `pdftotext -f/-l`；兜底 title+abstract | `version_id?` · `page_start/end` · `max_chars`（默认 5 万） |
| `get_node_relations` | 有向断言 + 引用 + 各维相似邻居 + 邻居标题 | `direction` · `relation_types` |

**接入步骤**：

1. 设置 → Codex / MCP access token 创建 token（明文只显示一次；泄露即撤销）。
2. `cd server && cargo build --release -p epistemic-mcp`
3. 注册（以 Codex 为例，Claude Code 用 `claude mcp add` 同理）：

```bash
codex mcp add epistemic \
  --env EPISTEMIC_MCP_TOKEN=epm_xxx \
  --env DATABASE_URL=postgres://epistemic:epistemic@localhost:5432/epistemic \
  --env PDF_DIR=/abs/path/Epistemic/data/pdfs \
  --env TEI_DIR=/abs/path/Epistemic/data/tei \
  -- /abs/path/Epistemic/server/target/release/epistemic-mcp
```

---

## 10. 部署与运维（deploy/）

docker-compose 四个服务（MCP 不容器化，是宿主机 stdio 侧车）：

| 服务 | 镜像/构建 | 端口 | 卷 |
|---|---|---|---|
| postgres | `pgvector/pgvector:pg16` | 5432 | `pgdata` |
| api | `server/Dockerfile`（`BIN=epistemic-api`） | 8080 | `pdfs:/data/pdfs` `tei:/data/tei` |
| worker | 同上（`BIN=epistemic-worker`） | — | 同上 |
| caddy | `caddy:2` | 80/443 | Caddyfile + `web/dist`（只读挂载）|

- Caddy：`$DOMAIN` 设真实域名即自动 HTTPS；`/api/*`、`/health` 反代 api，其余 SPA fallback。前端需先 `npm run build`。
- 运行时镜像装了 `poppler-utils`（pdftoppm / pdftotext）。api 启动自动跑 migration；users 表为空且设了 `BOOTSTRAP_ADMIN_EMAIL` 时自动建管理员。
- 备份：`deploy/backup.sh` = `pg_dump --format=custom` + PDF 目录 tar.gz，写入 `$BACKUP_ROOT`；offsite（rclone）留了注释位，需自行 cron。
- PDF 只经鉴权端点访问，磁盘目录不对外；登录接口限速；robots 禁抓。

### 环境变量（代码实际读取的）

| 组 | 变量（默认值） |
|---|---|
| 数据库 | `DATABASE_URL`（postgres://epistemic:epistemic@localhost:5432/epistemic） |
| API | `API_HOST`(0.0.0.0) · `API_PORT`(8080) · `SESSION_SECURE`(false) · `RUST_LOG` |
| 初始管理员 | `BOOTSTRAP_ADMIN_EMAIL`（触发开关）· `BOOTSTRAP_ADMIN_NAME`(Admin) · `BOOTSTRAP_ADMIN_PASSWORD`(changeme123) |
| 存储 | `PDF_DIR`(./data/pdfs) · `TEI_DIR`(./data/tei) |
| LLM | `OPENAI_API_KEY`（或 `LLM_API_KEY`，必填）· `OPENAI_MODEL`(gpt-4o) · `OPENAI_BASE_URL`(https://api.openai.com/v1) · `LLM_TIMEOUT_SECS`(1800) |
| PDF 渲染 | `PDF_RENDER_DPI`(150) · `PDF_MAX_PAGES`(不限) |
| Embedding | `EMBEDDING_API_KEY`（或 `SILICONFLOW_API_KEY`，必填）· `EMBEDDING_MODEL`(Qwen/Qwen3-Embedding-8B) · `EMBEDDING_BASE_URL`(https://api.siliconflow.cn/v1) · `EMBEDDING_DIM`（校验用，可不设）· `EMBEDDING_SEND_DIM`(false，是否随请求发 dimensions) |
| Worker | `WORKER_CONCURRENCY`(16) |
| MCP | `EPISTEMIC_MCP_TOKEN`（必填） |
| 部署 | `DOMAIN`(localhost) · `BACKUP_ROOT`(./backups) |

> 注：`.env.example` 里的 `SESSION_SECRET`、`EMBEDDING_PROVIDER` 代码并不读取，属历史遗留。

### scripts/

- `seed_fixtures.sql`：幂等种子 11 篇经典论文（Transformer/BERT/GPT-3/InstructGPT/ResNet/VGG/VAE/GAN/Adam/BPE/Bahdanau）+ 样例 DNA、关系、证据，供前端开发与演示；需先有用户。
- `requeue_aspect_pipeline.sql`：给全库每篇论文重新入队 `extract_dna`（改分面管线后全量重跑）。

---

## 11. 测试集与评估（testset/）

- 金标 `gold/*.json` 当前 **11 篇**（与 seed_fixtures 对齐），每篇标注：`dna.claims[]`（text+source_text+page）、`dna.methods[]`、`relations[]`（type/aspect/target_arxiv/explanation/evidence）、`citation_contexts[]`（cited_arxiv/sentence/gold_type/confidence_band）。多层 aspect 尚未进金标。
- `eval_gold.py`：① schema 校验（字段齐全、关系类型在 12 种白名单内）；② `--db` 模式对照数据库算**关系覆盖率**（gold 关系有多少已入库，列出 miss）。精确的 precision/recall 评估待管线数据积累后补。
- 质量门槛沿用原则：先测再扩，不达标就收缩关系类型而不是灌低质量边。

---

## 12. 开发约定

- 提交前必过：`cargo fmt` + `clippy -D warnings`；前端 `tsc --noEmit`。
- migration 用 `sqlx migrate`，**只增不改**已合并的 migration。
- C 线（AI 管线）一律通过 core crate 的 repository 函数写库，不直接拼 SQL；关系状态变更走 repo 函数内联动写 jobs。
- main + 短生命 feature 分支，PR 需另一人过目（AI 生成的代码更要看）。
- 单测范围：家族匹配/去重/距离计算/前端图纯函数；AI 质量看测集指标；E2E 后置。
- 规模假设：1 个组织、≤20 用户、≤5000 篇、≤10 万关系——单 Postgres 足够。

### 三人分工

| 线 | 范围 |
|---|---|
| A 后端内核 | schema/migrations、认证邀请、works/groups/graphs CRUD、图查询、合并拆分 |
| B 前端 | 地图 / Ego / 卡片 / PDF / 审核队列 / 组图导航 / 评论 |
| C AI 管线 | worker、prompts 与 schemas、VLM 抽取、引文分类、embedding、距离引擎、测集评估 |

---

## 13. 已知差距与下一步（诚实清单）

当前实现与目标之间还差的、按重要度排：

1. **证据页码质量依赖取源路径**：PDF→VLM 路径页码可信（图片序号）；HTML 路径页码可能为 0，bbox 多数缺失——前端已用文本搜索回退定位，但"精确高亮"只在有 bbox 时成立。
2. 金标测试集 11 篇（目标 20–30），标注偏稀疏；precision/recall 评估未启动。
3. 导入页预览是原始 JSON；项目页只有覆盖统计；组/图无重命名删除、成员管理 UI 不完整。
4. Ego 视图溢出组展开是客户端桩节点；地图断言边点击跳审核队列的 `?focus=` 参数队列页未消费。
5. topic 维度（旧主题引力）默认权重 0 且无滑杆，实际处于休眠状态。
6. PDF 阅读器全页急渲染，长 PDF 会重；批注跳转只滚动不高亮。
7. 超边（组合关系）数据模型已支持，UI 未开放。
8. backup.sh 未接 cron / offsite；`.env.example` 与代码实际读取的变量有两处出入（见 §10）。

原 PRD 的十步验收演示与 Go/No-Go 标准（≥3 人每周主动使用、AI 候选接受率 ≥60%、组内共识优于 Zotero+群聊+Notion）继续作为 dogfood 阶段的评判标准。
