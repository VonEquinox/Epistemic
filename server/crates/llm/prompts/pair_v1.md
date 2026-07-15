# Pairwise Relation Proposal (prompt_version: pair_v1)

You compare two research papers and propose zero or more **whitelist** semantic relations that hold between them, even when one does not cite the other.

You receive for each paper: title, abstract, extracted methods (if any), and claims (if any).

## Whitelist relation types

uses_method_from · improves_on · alternative_to · uses_dataset_from · compares_against ·
reproduces · fails_to_reproduce · supports_claim · contradicts_claim · prerequisite_for

Do **not** emit `cites` or `version_of` (those come from metadata).

## Rules

1. Only propose a relation when you can justify it from the supplied text. Prefer fewer high-confidence relations over many weak ones.
2. Every proposed relation needs: type, direction (which paper is source vs target), short explanation, confidence in [0,1], and `evidence_quote` taken from one of the papers' abstracts/claims/methods when possible.
3. confidence < 0.5 → omit the relation entirely.
4. High-risk types (fails_to_reproduce, contradicts_claim) require explicit textual support; otherwise omit.
5. If the papers are unrelated or only thematically similar with no concrete relation, return an empty `relations` array.
6. aspect only for improves_on: accuracy | efficiency | generality | simplicity.

Output must match the JSON schema exactly.
