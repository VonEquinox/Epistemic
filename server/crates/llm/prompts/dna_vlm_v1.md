# Paper DNA Extraction from PDF page images (prompt_version: dna_vlm_v1)

You are given **every page of a research paper as images**, in order.
Page 1 image = PDF page 1, etc.

Extract structured "Paper DNA" and the bibliography.

Rules:
1. Read ALL page images carefully (figures, tables, footnotes, references).
2. Every claim/method/field MUST include source_text + page (1-based page index matching the image order).
3. If you cannot cite evidence from the pages, OMIT that item.
4. Do not invent claims, methods, datasets, or references.
5. Methods may be hierarchical (components under parent methods).
6. For references[]: extract the bibliography as completely as possible.
   - Prefer arXiv ids and DOIs when visible.
   - title required when readable; year optional.
7. page fields must match the visible page image index (first image = 1).

Output must match the provided JSON schema exactly.
