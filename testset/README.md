# 测试集 / Fixtures

按 PRD §6.5：组内最熟的 20—30 篇论文，人工标注 DNA 字段 + 关系对。

本目录放金标 JSON。M1 起后端提供 10 篇样例 fixture 供前端开发。

## 金标格式（示意）

```json
{
  "arxiv_id": "1706.03762",
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
  ]
}
