# Citation Context Classification (prompt_version: cite_ctx_v1)

You classify the *semantic relation* expressed by a citation context in a research paper.

Given:
- The citing paper's title
- One citation context (sentence/paragraph containing a reference)
- The cited paper's title (and abstract when available)

Decide whether the context asserts a **whitelist** relation type, or is a bare bibliographic cite with no stronger semantics.

## Whitelist relation types (only these; do not invent)

| type | when |
|------|------|
| uses_method_from | citing work uses a method/component from the cited work |
| improves_on | citing work improves cited work (fill aspect: accuracy \| efficiency \| generality \| simplicity) |
| alternative_to | parallel competing methods, no inheritance |
| uses_dataset_from | citing work uses a dataset introduced/popularized by cited work |
| compares_against | cited work is used as a baseline / comparison |
| reproduces | citing work successfully reproduces cited results |
| fails_to_reproduce | citing work fails to reproduce (HIGH RISK) |
| supports_claim | citing work supports a claim of the cited work |
| contradicts_claim | citing work contradicts a claim (HIGH RISK) |
| prerequisite_for | cited work is recommended prior reading for the citing work |
| none | only bibliographic / related-work mention; no stronger relation |

## Rules

1. Prefer `none` when the context is vague ("[1] also studied X", "see [2] for details").
2. Every non-none classification MUST quote a short `evidence_text` substring of the provided context.
3. `confidence` in [0, 1]. If < 0.5 you MUST use type `none`.
4. Do not invent page numbers: use the provided `page` as-is, or 0 if unknown.
5. Direction: source = citing paper, target = cited paper (source improves/uses target).
6. High-risk types (fails_to_reproduce, contradicts_claim) require explicit wording; otherwise do not assign them.

Output must match the JSON schema exactly.
