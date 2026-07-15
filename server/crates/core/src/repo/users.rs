use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{Invite, User, UserPublic, UserRole};
use crate::error::{AppError, AppResult};
use crate::util::{hash_password, random_token, verify_password};

pub async fn create_user(
    pool: &PgPool,
    email: &str,
    name: &str,
    password: &str,
    role: UserRole,
) -> AppResult<UserPublic> {
    let password_hash = hash_password(password)
        .map_err(|e| AppError::Other(anyhow::anyhow!("hash error: {e}")))?;

    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (email, name, password_hash, role)
        VALUES ($1, $2, $3, $4)
        RETURNING id, email, name, password_hash, role, created_at
        "#,
    )
    .bind(email)
    .bind(name)
    .bind(password_hash)
    .bind(role)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint() == Some("users_email_key") => {
            AppError::Conflict("email already registered".into())
        }
        _ => e.into(),
    })?;

    Ok(user.into())
}

pub async fn find_by_email(pool: &PgPool, email: &str) -> AppResult<Option<User>> {
    let user = sqlx::query_as::<_, User>(
        r#"SELECT id, email, name, password_hash, role, created_at FROM users WHERE email = $1"#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<UserPublic>> {
    let user = sqlx::query_as::<_, User>(
        r#"SELECT id, email, name, password_hash, role, created_at FROM users WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(user.map(Into::into))
}

pub async fn authenticate(pool: &PgPool, email: &str, password: &str) -> AppResult<UserPublic> {
    let user = find_by_email(pool, email)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if !verify_password(password, &user.password_hash) {
        return Err(AppError::Unauthorized);
    }
    Ok(user.into())
}

pub async fn list_users(pool: &PgPool) -> AppResult<Vec<UserPublic>> {
    let rows = sqlx::query_as::<_, User>(
        r#"SELECT id, email, name, password_hash, role, created_at FROM users ORDER BY created_at"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn create_invite(pool: &PgPool, email: &str, created_by: Uuid) -> AppResult<Invite> {
    let token = random_token();
    let invite = sqlx::query_as::<_, Invite>(
        r#"
        INSERT INTO invites (email, token, created_by)
        VALUES ($1, $2, $3)
        RETURNING id, email, token, created_by, used_at, created_at
        "#,
    )
    .bind(email)
    .bind(token)
    .bind(created_by)
    .fetch_one(pool)
    .await?;
    Ok(invite)
}

pub async fn find_invite_by_token(pool: &PgPool, token: &str) -> AppResult<Option<Invite>> {
    let invite = sqlx::query_as::<_, Invite>(
        r#"SELECT id, email, token, created_by, used_at, created_at FROM invites WHERE token = $1"#,
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;
    Ok(invite)
}

pub async fn mark_invite_used(pool: &PgPool, id: Uuid) -> AppResult<()> {
    sqlx::query(r#"UPDATE invites SET used_at = now() WHERE id = $1"#)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn bootstrap_admin(
    pool: &PgPool,
    email: &str,
    name: &str,
    password: &str,
) -> AppResult<Option<UserPublic>> {
    let count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM users"#)
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(None);
    }
    Ok(Some(
        create_user(pool, email, name, password, UserRole::Admin).await?,
    ))
}
