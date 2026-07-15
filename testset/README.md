# 测试集 / Fixtures

按 PRD §6.5：组内最熟的 20—30 篇论文，人工标注 DNA 字段 + 关系对。

- `gold/` — 金标 JSON（当前 11 篇，与 `scripts/seed_fixtures.sql` 对齐）
- M1 起后端 seed 提供样例供前端开发

## 金标格式

```json
{
  "arxiv_id": "1706.03762",
  "title": "...",
  "year": 2017,
  "dna": {
    "claims": [{ "text": "...", "source_text": "...", "page": 2 }],
    "methods": [{ "name": "Transformer", "source_text": "...", "page": 2 }]
  },
  "relations": [
    {
      "type": "improves_on",
      "aspect": "accuracy",
      "target_arxiv": "1409.0473",
      "explanation": "...",
      "evidence": [{ "page": 1, "text": "..." }]
    }
  ],
  "citation_contexts": [
    {
      "cited_arxiv": "1409.0473",
      "sentence": "...",
      "gold_type": "uses_method_from",
      "confidence_band": "high"
    }
  ]
}
```

## 轻量评估

```bash
# 校验金标 schema 完整性（无 LLM）
python3 testset/eval_gold.py

# 若 API 已 seed，可对比 DB 中已写入关系（需 DATABASE_URL）
python3 testset/eval_gold.py --db
```

评估指标（M3 轻量版）：

| 任务 | 指标 |
|------|------|
| DNA claims/methods | 字段存在率（占位，待模型对照） |
| 关系对 | gold 关系是否在 seed/DB 中出现（type + 两端 arxiv） |
| 引文上下文 | gold_type 枚举合法性 |

完整 precision/recall 在 worker 跑通 LLM 后接入。
