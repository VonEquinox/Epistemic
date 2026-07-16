use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{Job, JobStatus};
use crate::error::AppResult;

pub async fn enqueue(pool: &PgPool, kind: &str, payload: serde_json::Value) -> AppResult<Job> {
    let job = sqlx::query_as::<_, Job>(
        r#"
        INSERT INTO jobs (kind, payload)
        VALUES ($1, $2)
        RETURNING id, kind, payload, status, attempts, run_after,
                  locked_by, locked_at, last_error, created_at
        "#,
    )
    .bind(kind)
    .bind(payload)
    .fetch_one(pool)
    .await?;
    Ok(job)
}

pub async fn enqueue_many(
    pool: &PgPool,
    items: &[(String, serde_json::Value)],
) -> AppResult<Vec<Uuid>> {
    let mut ids = Vec::with_capacity(items.len());
    for (kind, payload) in items {
        let job = enqueue(pool, kind, payload.clone()).await?;
        ids.push(job.id);
    }
    Ok(ids)
}

pub async fn claim_next(pool: &PgPool, worker_id: &str) -> AppResult<Option<Job>> {
    let mut tx = pool.begin().await?;

    // Prefer DNA / embed / pairing over bulk metadata crawl so one paper can finish
        // end-to-end instead of crawling all 80 HTMLs first.
        let row = sqlx::query_as::<_, Job>(
        r#"
        SELECT id, kind, payload, status, attempts, run_after,
               locked_by, locked_at, last_error, created_at
        FROM jobs
        WHERE status = 'queued' AND run_after <= now()
        ORDER BY
          CASE kind
            WHEN 'extract_dna' THEN 0
            WHEN 'embed' THEN 0
            WHEN 'propose_pairs' THEN 2
            WHEN 'update_neighbors_citation' THEN 2
            WHEN 'update_neighbors_lineage' THEN 2
            WHEN 'resolve_metadata' THEN 3
            WHEN 'fetch_pdf' THEN 4
            WHEN 'fetch_references' THEN 4
            ELSE 5
          END,
          created_at
        FOR UPDATE SKIP LOCKED
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut *tx)
    .await?;

    let Some(job) = row else {
        tx.commit().await?;
        return Ok(None);
    };

    let updated = sqlx::query_as::<_, Job>(
        r#"
        UPDATE jobs
        SET status = 'running',
            locked_by = $2,
            locked_at = now(),
            attempts = attempts + 1
        WHERE id = $1
        RETURNING id, kind, payload, status, attempts, run_after,
                  locked_by, locked_at, last_error, created_at
        "#,
    )
    .bind(job.id)
    .bind(worker_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Some(updated))
}

pub async fn mark_done(pool: &PgPool, id: Uuid) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE jobs
        SET status = 'done', locked_by = NULL, locked_at = NULL, last_error = NULL
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_failed(
    pool: &PgPool,
    id: Uuid,
    error: &str,
    attempts: i32,
    max_attempts: i32,
) -> AppResult<()> {
    if attempts >= max_attempts {
        sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'failed', last_error = $2, locked_by = NULL, locked_at = NULL
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(error)
        .execute(pool)
        .await?;
    } else {
        let backoff_mins = 1i64 << attempts.min(6);
        let run_after: DateTime<Utc> = Utc::now() + Duration::minutes(backoff_mins);
        sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'queued', last_error = $2, run_after = $3,
                locked_by = NULL, locked_at = NULL
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(error)
        .bind(run_after)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn jobs_for_version(pool: &PgPool, version_id: Uuid) -> AppResult<Vec<Job>> {
    let rows = sqlx::query_as::<_, Job>(
        r#"
        SELECT id, kind, payload, status, attempts, run_after,
               locked_by, locked_at, last_error, created_at
        FROM jobs
        WHERE payload->>'version_id' = $1
        ORDER BY created_at
        "#,
    )
    .bind(version_id.to_string())
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Jobs for a work: payload work_id match OR any of its versions.
pub async fn jobs_for_work(pool: &PgPool, work_id: Uuid) -> AppResult<Vec<Job>> {
    let rows = sqlx::query_as::<_, Job>(
        r#"
        SELECT id, kind, payload, status, attempts, run_after,
               locked_by, locked_at, last_error, created_at
        FROM jobs
        WHERE payload->>'work_id' = $1
           OR payload->>'version_id' IN (
                SELECT id::text FROM versions WHERE work_id = $2
           )
        ORDER BY created_at
        "#,
    )
    .bind(work_id.to_string())
    .bind(work_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Re-enqueue a pipeline step for a version/work (failed or manual re-run).
pub async fn requeue(
    pool: &PgPool,
    kind: &str,
    version_id: Option<Uuid>,
    work_id: Option<Uuid>,
) -> AppResult<Job> {
    let mut payload = serde_json::Map::new();
    if let Some(v) = version_id {
        payload.insert("version_id".into(), serde_json::json!(v));
    }
    if let Some(w) = work_id {
        payload.insert("work_id".into(), serde_json::json!(w));
    }
    if payload.is_empty() {
        return Err(crate::error::AppError::BadRequest(
            "requeue requires version_id or work_id".into(),
        ));
    }
    enqueue(pool, kind, serde_json::Value::Object(payload)).await
}

// silence unused import warning if JobStatus only used via FromRow
#[allow(dead_code)]
fn _use_job_status(_: JobStatus) {}
