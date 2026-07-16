-- Research groups (teams) + graphs (maps) under each group.
-- Global library (works) remains shared; graphs are membership-scoped views.

CREATE TYPE group_role AS ENUM ('owner', 'admin', 'member');

CREATE TABLE research_groups (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE group_members (
    group_id   UUID NOT NULL REFERENCES research_groups(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role       group_role NOT NULL DEFAULT 'member',
    joined_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (group_id, user_id)
);

CREATE INDEX group_members_user_idx ON group_members (user_id);

CREATE TABLE graphs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id    UUID NOT NULL REFERENCES research_groups(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX graphs_group_idx ON graphs (group_id);

CREATE TABLE graph_works (
    graph_id   UUID NOT NULL REFERENCES graphs(id) ON DELETE CASCADE,
    work_id    UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    added_by   UUID REFERENCES users(id) ON DELETE SET NULL,
    added_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (graph_id, work_id)
);

CREATE INDEX graph_works_work_idx ON graph_works (work_id);

-- Seed: one default group + one default graph containing all current works.
-- Every existing user becomes a member so nothing breaks for the current demo.
DO $$
DECLARE
  gid UUID;
  graph_id UUID;
  admin_id UUID;
BEGIN
  SELECT id INTO admin_id FROM users ORDER BY created_at LIMIT 1;

  INSERT INTO research_groups (id, name, description, created_by)
  VALUES (
    'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb',
    '默认研究组',
    '系统迁移创建的默认组：包含库内全部论文的主图',
    admin_id
  )
  ON CONFLICT (id) DO NOTHING
  RETURNING id INTO gid;

  IF gid IS NULL THEN
    gid := 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb'::uuid;
  END IF;

  INSERT INTO group_members (group_id, user_id, role)
  SELECT gid, u.id,
         CASE WHEN u.role = 'admin' THEN 'owner'::group_role ELSE 'member'::group_role END
  FROM users u
  ON CONFLICT DO NOTHING;

  INSERT INTO graphs (id, group_id, name, description, created_by)
  VALUES (
    'cccccccc-cccc-cccc-cccc-cccccccccccc',
    gid,
    '主图',
    '默认图：导入库内全部论文',
    admin_id
  )
  ON CONFLICT (id) DO NOTHING
  RETURNING id INTO graph_id;

  IF graph_id IS NULL THEN
    graph_id := 'cccccccc-cccc-cccc-cccc-cccccccccccc'::uuid;
  END IF;

  INSERT INTO graph_works (graph_id, work_id, added_by)
  SELECT graph_id, w.id, admin_id
  FROM works w
  ON CONFLICT DO NOTHING;
END $$;
