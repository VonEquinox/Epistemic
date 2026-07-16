-- Re-run multi-aspect DNA extract for all works with a primary version.
-- extract_dna will upsert paper_aspects and enqueue embed (8 vectors + neighbors).
INSERT INTO jobs (kind, payload, status)
SELECT
  'extract_dna',
  jsonb_build_object(
    'version_id', primary_version_id,
    'work_id', id
  ),
  'queued'
FROM works
WHERE primary_version_id IS NOT NULL;
