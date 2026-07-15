-- Seed well-known ML papers as fixtures for UI development.
-- Usage:
--   1. Start API once with BOOTSTRAP_ADMIN_* so a user exists
--   2. psql "$DATABASE_URL" -f scripts/seed_fixtures.sql

BEGIN;

INSERT INTO projects (id, name, description)
VALUES ('aaaaaaaa-bbbb-cccc-dddd-000000000001', 'Foundations', '经典基础论文')
ON CONFLICT (id) DO NOTHING;

-- Create works/versions via a DO block
DO $seed$
DECLARE
  uid UUID;
  wid UUID;
  vid UUID;
  aid UUID;
  rid UUID;
  attn UUID;
  bert UUID;
  i INT;
  a TEXT;
  tnorm TEXT;
  rec RECORD;
BEGIN
  SELECT id INTO uid FROM users ORDER BY created_at LIMIT 1;
  IF uid IS NULL THEN
    RAISE EXCEPTION 'no users; start API once with BOOTSTRAP_ADMIN_* first';
  END IF;

  FOR rec IN
    SELECT * FROM (VALUES
      ('1706.03762', 'Attention Is All You Need', 2017,
       ARRAY['Ashish Vaswani','Noam Shazeer','Niki Parmar','Jakob Uszkoreit','Llion Jones','Aidan N. Gomez','Lukasz Kaiser','Illia Polosukhin']::text[],
       'We propose a new simple network architecture, the Transformer, based solely on attention mechanisms.'),
      ('1810.04805', 'BERT: Pre-training of Deep Bidirectional Transformers for Language Understanding', 2019,
       ARRAY['Jacob Devlin','Ming-Wei Chang','Kenton Lee','Kristina Toutanova']::text[],
       'We introduce a new language representation model called BERT.'),
      ('2005.14165', 'Language Models are Few-Shot Learners', 2020,
       ARRAY['Tom B. Brown','Benjamin Mann','Nick Ryder','Melanie Subbiah','Jared Kaplan']::text[],
       'We train GPT-3 and find that scaling up language models greatly improves task-agnostic few-shot performance.'),
      ('2203.02155', 'Training language models to follow instructions with human feedback', 2022,
       ARRAY['Long Ouyang','Jeff Wu','Xu Jiang','Diogo Almeida','Carroll L. Wainwright']::text[],
       'We show an avenue for aligning language models with user intent by fine-tuning with human feedback.'),
      ('1409.0473', 'Neural Machine Translation by Jointly Learning to Align and Translate', 2015,
       ARRAY['Dzmitry Bahdanau','Kyunghyun Cho','Yoshua Bengio']::text[],
       'We conjecture that the use of a fixed-length vector is a bottleneck in improving NMT.'),
      ('1508.07909', 'Neural Machine Translation of Rare Words with Subword Units', 2016,
       ARRAY['Rico Sennrich','Barry Haddow','Alexandra Birch']::text[],
       'We introduce a simpler and more effective approach, making open-vocabulary NMT possible via BPE.'),
      ('1412.6980', 'Adam: A Method for Stochastic Optimization', 2015,
       ARRAY['Diederik P. Kingma','Jimmy Ba']::text[],
       'We introduce Adam, an algorithm for first-order gradient-based optimization of stochastic objective functions.'),
      ('1512.03385', 'Deep Residual Learning for Image Recognition', 2016,
       ARRAY['Kaiming He','Xiangyu Zhang','Shaoqing Ren','Jian Sun']::text[],
       'We present a residual learning framework to ease the training of networks that are substantially deeper.'),
      ('1409.1556', 'Very Deep Convolutional Networks for Large-Scale Image Recognition', 2015,
       ARRAY['Karen Simonyan','Andrew Zisserman']::text[],
       'We investigate the effect of the convolutional network depth on its accuracy.'),
      ('1312.6114', 'Auto-Encoding Variational Bayes', 2014,
       ARRAY['Diederik P Kingma','Max Welling']::text[],
       'We introduce a stochastic variational inference and learning algorithm that scales to large datasets.'),
      ('1406.2661', 'Generative Adversarial Nets', 2014,
       ARRAY['Ian J. Goodfellow','Jean Pouget-Abadie','Mehdi Mirza','Bing Xu','David Warde-Farley','Sherjil Ozair','Aaron Courville','Yoshua Bengio']::text[],
       'We propose a new framework for estimating generative models via an adversarial process.')
    ) AS t(arxiv_id, title, year, authors, abstract)
  LOOP
    IF EXISTS (SELECT 1 FROM versions WHERE arxiv_id = rec.arxiv_id) THEN
      CONTINUE;
    END IF;

    tnorm := lower(regexp_replace(rec.title, '[^a-zA-Z0-9 ]', '', 'g'));
    tnorm := regexp_replace(trim(tnorm), '\s+', ' ', 'g');

    INSERT INTO works (title_norm, created_by)
    VALUES (tnorm, uid) RETURNING id INTO wid;

    INSERT INTO versions (work_id, kind, arxiv_id, title, abstract, year, venue_name, metadata_source)
    VALUES (wid, 'arxiv', rec.arxiv_id, rec.title, rec.abstract, rec.year, 'arXiv', 'fixture')
    RETURNING id INTO vid;

    UPDATE works SET primary_version_id = vid WHERE id = wid;

    i := 0;
    FOREACH a IN ARRAY rec.authors LOOP
      SELECT id INTO aid FROM authors WHERE full_name = a LIMIT 1;
      IF aid IS NULL THEN
        INSERT INTO authors (full_name) VALUES (a) RETURNING id INTO aid;
      END IF;
      INSERT INTO version_authors (version_id, author_id, position)
      VALUES (vid, aid, i) ON CONFLICT DO NOTHING;
      i := i + 1;
    END LOOP;

    INSERT INTO work_projects (work_id, project_id)
    VALUES (wid, 'aaaaaaaa-bbbb-cccc-dddd-000000000001')
    ON CONFLICT DO NOTHING;

    IF rec.arxiv_id = '1706.03762' THEN
      INSERT INTO claims (work_id, text, source, review_status, model_version)
      VALUES (wid,
        'Self-attention alone is sufficient for state-of-the-art machine translation.',
        'ai_candidate', 'unreviewed', 'fixture/dna_v1');
      INSERT INTO methods (work_id, name, description, source, review_status, model_version)
      VALUES (wid, 'Transformer',
        'Encoder-decoder with multi-head self-attention',
        'ai_candidate', 'unreviewed', 'fixture/dna_v1');
    END IF;
  END LOOP;

  SELECT w.id INTO attn FROM works w
    JOIN versions v ON v.id = w.primary_version_id WHERE v.arxiv_id = '1706.03762';
  SELECT w.id INTO bert FROM works w
    JOIN versions v ON v.id = w.primary_version_id WHERE v.arxiv_id = '1810.04805';

  IF attn IS NOT NULL AND bert IS NOT NULL
     AND NOT EXISTS (
       SELECT 1 FROM relations r
       JOIN relation_members s ON s.relation_id = r.id AND s.role = 'source' AND s.entity_id = bert
       JOIN relation_members t ON t.relation_id = r.id AND t.role = 'target' AND t.entity_id = attn
       WHERE r.type = 'improves_on'
     )
  THEN
    INSERT INTO relations (type, aspect, explanation, confidence, source, review_status, model_version)
    VALUES ('improves_on', 'accuracy',
      'BERT applies bidirectional Transformer encoders for pretraining, improving downstream NLP accuracy.',
      0.88, 'ai_candidate', 'unreviewed', 'fixture')
    RETURNING id INTO rid;

    INSERT INTO relation_members (relation_id, entity_kind, entity_id, role, anchor_work_id, position)
    VALUES
      (rid, 'work', bert, 'source', bert, 0),
      (rid, 'work', attn, 'target', attn, 1);

    INSERT INTO evidence_spans (relation_id, version_id, page, text, extraction_field)
    SELECT rid, v.id, 1,
      'BERT is designed to pre-train deep bidirectional representations',
      'relation'
    FROM versions v WHERE v.arxiv_id = '1810.04805';
  END IF;
END
$seed$;

COMMIT;
