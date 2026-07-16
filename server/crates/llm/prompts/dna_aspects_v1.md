# Paper DNA multi-aspect extraction (prompt_version: dna_aspects_v1)

You are given the **complete text** of a research paper (from HTML/PDF body).

Extract a fixed set of **8 analysis aspects** plus the bibliography.

## Aspects (all required keys)

For each aspect produce:
- `summary`: 1–4 sentences capturing that layer (used for embedding). Empty string only if the paper truly has nothing for that layer.
- `bullets`: short bullet strings (0–8 items) with concrete names/techniques/numbers when available.
- `source_text`: a short verbatim quote supporting the summary (may be empty if page unknown).
- `page`: integer page index when known, else 0.

| key | What to extract |
|---|---|
| `problem` | Research question / problem setting / motivation |
| `contributions` | Claimed contributions / novelty bullets |
| `methods` | Core methods, algorithms, architectures, training procedures |
| `theory` | Theoretical results, formalization, proofs, complexity (empty ok for pure empirics) |
| `datasets` | Datasets, benchmarks, evaluation protocol, metrics |
| `findings` | Main empirical/theoretical findings and claims |
| `limitations` | Limitations, failure modes, threats to validity |
| `positioning` | Related work positioning: what it builds on / differs from |

## Rules

1. Read the full paper body (not only abstract).
2. Do not invent facts; omit bullets rather than hallucinate.
3. `summary` should be self-contained enough to compare two papers on that aspect alone.
4. For `references[]`: extract bibliography as completely as possible (title, arxiv_id, doi, year).
5. Output must match the provided JSON schema exactly.
