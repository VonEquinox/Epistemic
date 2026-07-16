//! research_groups + graphs (maps under a group).

use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{
    Graph, GraphWithMeta, GroupMember, GroupRole, ResearchGroup, ResearchGroupWithMeta,
};
use crate::error::{AppError, AppResult};

pub async fn create_group(
    pool: &PgPool,
    name: &str,
    description: &str,
    creator_id: Uuid,
) -> AppResult<ResearchGroup> {
    let mut tx = pool.begin().await?;
    let g = sqlx::query_as::<_, ResearchGroup>(
        r#"
        INSERT INTO research_groups (name, description, created_by)
        VALUES ($1, $2, $3)
        RETURNING id, name, description, created_by, created_at
        "#,
    )
    .bind(name)
    .bind(description)
    .bind(creator_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO group_members (group_id, user_id, role)
        VALUES ($1, $2, 'owner')
        "#,
    )
    .bind(g.id)
    .bind(creator_id)
    .execute(&mut *tx)
    .await?;

    // Every new group starts with an empty "主图" so the UI has somewhere to go.
    sqlx::query(
        r#"
        INSERT INTO graphs (group_id, name, description, created_by)
        VALUES ($1, '主图', '组内默认图', $2)
        "#,
    )
    .bind(g.id)
    .bind(creator_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(g)
}

pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<ResearchGroupWithMeta>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
        description: String,
        created_by: Option<Uuid>,
        created_at: chrono::DateTime<chrono::Utc>,
        my_role: GroupRole,
        member_count: i64,
        graph_count: i64,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT g.id, g.name, g.description, g.created_by, g.created_at,
               gm.role AS my_role,
               (SELECT COUNT(*) FROM group_members m WHERE m.group_id = g.id) AS member_count,
               (SELECT COUNT(*) FROM graphs gr WHERE gr.group_id = g.id) AS graph_count
        FROM research_groups g
        JOIN group_members gm ON gm.group_id = g.id AND gm.user_id = $1
        ORDER BY g.created_at
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ResearchGroupWithMeta {
            group: ResearchGroup {
                id: r.id,
                name: r.name,
                description: r.description,
                created_by: r.created_by,
                created_at: r.created_at,
            },
            my_role: r.my_role,
            member_count: r.member_count,
            graph_count: r.graph_count,
        })
        .collect())
}

pub async fn get_group(pool: &PgPool, group_id: Uuid) -> AppResult<ResearchGroup> {
    sqlx::query_as::<_, ResearchGroup>(
        r#"
        SELECT id, name, description, created_by, created_at
        FROM research_groups WHERE id = $1
        "#,
    )
    .bind(group_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("group {group_id}")))
}

pub async fn require_member(
    pool: &PgPool,
    group_id: Uuid,
    user_id: Uuid,
) -> AppResult<GroupRole> {
    let role: Option<GroupRole> = sqlx::query_scalar(
        r#"
        SELECT role FROM group_members
        WHERE group_id = $1 AND user_id = $2
        "#,
    )
    .bind(group_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    role.ok_or_else(|| AppError::Forbidden("not a group member".into()))
}

pub async fn list_members(pool: &PgPool, group_id: Uuid) -> AppResult<Vec<GroupMember>> {
    let rows = sqlx::query_as::<_, GroupMember>(
        r#"
        SELECT group_id, user_id, role, joined_at
        FROM group_members WHERE group_id = $1
        ORDER BY joined_at
        "#,
    )
    .bind(group_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct MemberPublic {
    pub user_id: Uuid,
    pub email: String,
    pub name: String,
    pub role: GroupRole,
    pub joined_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_members_public(pool: &PgPool, group_id: Uuid) -> AppResult<Vec<MemberPublic>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        user_id: Uuid,
        email: String,
        name: String,
        role: GroupRole,
        joined_at: chrono::DateTime<chrono::Utc>,
    }
    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT gm.user_id, u.email, u.name, gm.role, gm.joined_at
        FROM group_members gm
        JOIN users u ON u.id = gm.user_id
        WHERE gm.group_id = $1
        ORDER BY gm.joined_at
        "#,
    )
    .bind(group_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| MemberPublic {
            user_id: r.user_id,
            email: r.email,
            name: r.name,
            role: r.role,
            joined_at: r.joined_at,
        })
        .collect())
}

pub async fn add_member(
    pool: &PgPool,
    group_id: Uuid,
    user_id: Uuid,
    role: GroupRole,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO group_members (group_id, user_id, role)
        VALUES ($1, $2, $3)
        ON CONFLICT (group_id, user_id) DO UPDATE SET role = EXCLUDED.role
        "#,
    )
    .bind(group_id)
    .bind(user_id)
    .bind(role)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn create_graph(
    pool: &PgPool,
    group_id: Uuid,
    name: &str,
    description: &str,
    creator_id: Uuid,
) -> AppResult<Graph> {
    let g = sqlx::query_as::<_, Graph>(
        r#"
        INSERT INTO graphs (group_id, name, description, created_by)
        VALUES ($1, $2, $3, $4)
        RETURNING id, group_id, name, description, created_by, created_at
        "#,
    )
    .bind(group_id)
    .bind(name)
    .bind(description)
    .bind(creator_id)
    .fetch_one(pool)
    .await?;
    Ok(g)
}

pub async fn list_graphs(pool: &PgPool, group_id: Uuid) -> AppResult<Vec<GraphWithMeta>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        group_id: Uuid,
        name: String,
        description: String,
        created_by: Option<Uuid>,
        created_at: chrono::DateTime<chrono::Utc>,
        work_count: i64,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT gr.id, gr.group_id, gr.name, gr.description, gr.created_by, gr.created_at,
               (SELECT COUNT(*) FROM graph_works gw WHERE gw.graph_id = gr.id) AS work_count
        FROM graphs gr
        WHERE gr.group_id = $1
        ORDER BY gr.created_at
        "#,
    )
    .bind(group_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| GraphWithMeta {
            graph: Graph {
                id: r.id,
                group_id: r.group_id,
                name: r.name,
                description: r.description,
                created_by: r.created_by,
                created_at: r.created_at,
            },
            work_count: r.work_count,
        })
        .collect())
}

pub async fn get_graph(pool: &PgPool, graph_id: Uuid) -> AppResult<Graph> {
    sqlx::query_as::<_, Graph>(
        r#"
        SELECT id, group_id, name, description, created_by, created_at
        FROM graphs WHERE id = $1
        "#,
    )
    .bind(graph_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("graph {graph_id}")))
}

/// Ensure user is a member of the group that owns this graph; returns the graph.
pub async fn require_graph_access(
    pool: &PgPool,
    graph_id: Uuid,
    user_id: Uuid,
) -> AppResult<Graph> {
    let g = get_graph(pool, graph_id).await?;
    require_member(pool, g.group_id, user_id).await?;
    Ok(g)
}

pub async fn list_graph_work_ids(pool: &PgPool, graph_id: Uuid) -> AppResult<Vec<Uuid>> {
    let ids = sqlx::query_scalar::<_, Uuid>(
        r#"SELECT work_id FROM graph_works WHERE graph_id = $1"#,
    )
    .bind(graph_id)
    .fetch_all(pool)
    .await?;
    Ok(ids)
}

pub async fn add_works(
    pool: &PgPool,
    graph_id: Uuid,
    work_ids: &[Uuid],
    added_by: Uuid,
) -> AppResult<i64> {
    let mut n = 0i64;
    for wid in work_ids {
        let r = sqlx::query(
            r#"
            INSERT INTO graph_works (graph_id, work_id, added_by)
            VALUES ($1, $2, $3)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(graph_id)
        .bind(wid)
        .bind(added_by)
        .execute(pool)
        .await?;
        n += r.rows_affected() as i64;
    }
    Ok(n)
}

pub async fn remove_work(pool: &PgPool, graph_id: Uuid, work_id: Uuid) -> AppResult<()> {
    sqlx::query(r#"DELETE FROM graph_works WHERE graph_id = $1 AND work_id = $2"#)
        .bind(graph_id)
        .bind(work_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Add every library work into the graph (useful for cloning the default map).
pub async fn add_all_library_works(
    pool: &PgPool,
    graph_id: Uuid,
    added_by: Uuid,
) -> AppResult<i64> {
    let r = sqlx::query(
        r#"
        INSERT INTO graph_works (graph_id, work_id, added_by)
        SELECT $1, w.id, $2 FROM works w
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(graph_id)
    .bind(added_by)
    .execute(pool)
    .await?;
    Ok(r.rows_affected() as i64)
}
