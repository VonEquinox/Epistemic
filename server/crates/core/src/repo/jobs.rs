use chrono::{DateTime, Duration, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::domain::{Job, JobStatus};
use crate::error::{AppError, AppResult};

const JOB_COLUMNS: &str =
    "id, kind, payload, status, attempts, run_after, locked_by, locked_at, last_error, created_at";

pub async fn enqueue(pool: &PgPool, kind: &str, payload: serde_json::Value) -> AppResult<Job> {
    enqueue_on(pool, kind, payload, None).await
}

pub async fn enqueue_unique(
    pool: &PgPool,
    kind: &str,
    payload: serde_json::Value,
    dedupe_key: &str,
) -> AppResult<Job> {
    enqueue_on(pool, kind, payload, Some(dedupe_key)).await
}

async fn enqueue_on<'e, E>(
    executor: E,
    kind: &str,
    payload: serde_json::Value,
    dedupe_key: Option<&str>,
) -> AppResult<Job>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let sql = format!(
        r#"
        INSERT INTO jobs (kind, payload, dedupe_key)
        VALUES ($1, $2, $3)
        ON CONFLICT (dedupe_key)
            WHERE dedupe_key IS NOT NULL AND status = 'queued'
        DO UPDATE SET payload = EXCLUDED.payload
        RETURNING {JOB_COLUMNS}
        "#
    );
    Ok(sqlx::query_as::<_, Job>(&sql)
        .bind(kind)
        .bind(payload)
        .bind(dedupe_key)
        .fetch_one(executor)
        .await?)
}

pub async fn enqueue_tx(
    conn: &mut PgConnection,
    kind: &str,
    payload: serde_json::Value,
) -> AppResult<Job> {
    enqueue_on(conn, kind, payload, None).await
}

pub async fn enqueue_unique_tx(
    conn: &mut PgConnection,
    kind: &str,
    payload: serde_json::Value,
    dedupe_key: &str,
) -> AppResult<Job> {
    enqueue_on(conn, kind, payload, Some(dedupe_key)).await
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

/// Claim one due job. A running job whose lease is older than two hours is
/// considered abandoned and can be atomically reclaimed.
pub async fn claim_next(pool: &PgPool, worker_id: &str) -> AppResult<Option<Job>> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query_as::<_, Job>(
        r#"
        SELECT id, kind, payload, status, attempts, run_after,
               locked_by, locked_at, last_error, created_at
        FROM jobs
        WHERE (status = 'queued' AND run_after <= now())
           OR (status = 'running' AND locked_at < now() - interval '2 hours')
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

pub async fn mark_done(pool: &PgPool, id: Uuid, worker_id: &str) -> AppResult<()> {
    let result = sqlx::query(
        r#"
        UPDATE jobs
        SET status = 'done', locked_by = NULL, locked_at = NULL, last_error = NULL
        WHERE id = $1 AND status = 'running' AND locked_by = $2
        "#,
    )
    .bind(id)
    .bind(worker_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::Conflict(format!(
            "job {id} lease is no longer owned by {worker_id}"
        )));
    }
    Ok(())
}

pub async fn mark_failed(
    pool: &PgPool,
    id: Uuid,
    worker_id: &str,
    error: &str,
    attempts: i32,
    max_attempts: i32,
) -> AppResult<()> {
    let result = if attempts >= max_attempts {
        sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'failed', last_error = $3, locked_by = NULL, locked_at = NULL
            WHERE id = $1 AND status = 'running' AND locked_by = $2
            "#,
        )
        .bind(id)
        .bind(worker_id)
        .bind(error)
        .execute(pool)
        .await?
    } else {
        let backoff_mins = 1i64 << attempts.min(6);
        let run_after: DateTime<Utc> = Utc::now() + Duration::minutes(backoff_mins);
        sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'queued', last_error = $3, run_after = $4,
                locked_by = NULL, locked_at = NULL
            WHERE id = $1 AND status = 'running' AND locked_by = $2
            "#,
        )
        .bind(id)
        .bind(worker_id)
        .bind(error)
        .bind(run_after)
        .execute(pool)
        .await?
    };
    if result.rows_affected() == 0 {
        return Err(AppError::Conflict(format!(
            "job {id} lease is no longer owned by {worker_id}"
        )));
    }
    Ok(())
}

/// Requeue the currently-owned job with updated payload and delay. This is used
/// by long-running orchestration jobs so polling does not create an unbounded
/// chain of fresh jobs.
pub async fn reschedule(
    pool: &PgPool,
    id: Uuid,
    worker_id: &str,
    payload: serde_json::Value,
    delay_secs: i64,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"
        UPDATE jobs
        SET status = 'queued', payload = $3,
            run_after = now() + make_interval(secs => $4::double precision),
            attempts = 0, locked_by = NULL, locked_at = NULL, last_error = NULL
        WHERE id = $1 AND status = 'running' AND locked_by = $2
        "#,
    )
    .bind(id)
    .bind(worker_id)
    .bind(payload)
    .bind(delay_secs as f64)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::Conflict(format!(
            "job {id} lease is no longer owned by {worker_id}"
        )));
    }
    Ok(())
}

pub async fn jobs_for_version(pool: &PgPool, version_id: Uuid) -> AppResult<Vec<Job>> {
    Ok(sqlx::query_as::<_, Job>(
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
    .await?)
}

pub async fn jobs_for_work(pool: &PgPool, work_id: Uuid) -> AppResult<Vec<Job>> {
    Ok(sqlx::query_as::<_, Job>(
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
    .await?)
}

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
        return Err(AppError::BadRequest(
            "requeue requires version_id or work_id".into(),
        ));
    }
    let scope = version_id.or(work_id).expect("payload checked above");
    enqueue_unique(
        pool,
        kind,
        serde_json::Value::Object(payload),
        &format!("manual:{kind}:{scope}"),
    )
    .await
}

#[allow(dead_code)]
fn _use_job_status(_: JobStatus) {}
