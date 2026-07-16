-- Qwen/Qwen3-Embedding-8B native dim is 4096 (SiliconFlow).
-- Safe on empty/dev DBs; if old 1024 rows exist they must be re-embedded.
ALTER TABLE embeddings
    ALTER COLUMN vec TYPE vector(4096);
