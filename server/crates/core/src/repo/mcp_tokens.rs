use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::UserPublic;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct McpTokenSummary {
    pub id: Uuid,
    pub name: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreatedMcpToken {
    #[serde(flatten)]
    pub summary: McpTokenSummary,
    /// Returned only once. Store it in the MCP client's environment.
    pub token: String,
}

fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

pub async fn create(pool: &PgPool, user_id: Uuid, name: &str) -> AppResult<CreatedMcpToken> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::Validation(
            "MCP token name must be 1-80 characters".into(),
        ));
    }
    let mut random = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut random);
    let token = format!("epm_{}", hex::encode(random));
    let hash = token_hash(&token);

    let summary = sqlx::query_as::<_, McpTokenSummary>(
        r#"
        INSERT INTO mcp_access_tokens (user_id, name, token_hash)
        VALUES ($1, $2, $3)
        RETURNING id, name, last_used_at, created_at
        "#,
    )
    .bind(user_id)
    .bind(name)
    .bind(hash)
    .fetch_one(pool)
    .await?;

    Ok(CreatedMcpToken { summary, token })
}

pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<McpTokenSummary>> {
    Ok(sqlx::query_as::<_, McpTokenSummary>(
        r#"
        SELECT id, name, last_used_at, created_at
        FROM mcp_access_tokens
        WHERE user_id = $1 AND revoked_at IS NULL
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

pub async fn revoke(pool: &PgPool, id: Uuid, user_id: Uuid) -> AppResult<()> {
    let result = sqlx::query(
        r#"
        UPDATE mcp_access_tokens SET revoked_at = now()
        WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
        "#,
    )
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("MCP token {id}")));
    }
    Ok(())
}

pub async fn authenticate(pool: &PgPool, token: &str) -> AppResult<UserPublic> {
    if !token.starts_with("epm_") || token.len() < 32 {
        return Err(AppError::Unauthorized);
    }
    let hash = token_hash(token);
    let mut tx = pool.begin().await?;
    let user = sqlx::query_as::<_, UserPublic>(
        r#"
        SELECT u.id, u.email, u.name, u.role, u.created_at
        FROM mcp_access_tokens t
        JOIN users u ON u.id = t.user_id
        WHERE t.token_hash = $1 AND t.revoked_at IS NULL
        FOR UPDATE OF t
        "#,
    )
    .bind(hash)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::Unauthorized)?;
    sqlx::query("UPDATE mcp_access_tokens SET last_used_at = now() WHERE token_hash = $1")
        .bind(token_hash(token))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(user)
}

#[cfg(test)]
mod tests {
    use super::token_hash;

    #[test]
    fn token_hash_is_stable_and_not_plaintext() {
        assert_eq!(token_hash("epm_test"), token_hash("epm_test"));
        assert_ne!(token_hash("epm_test"), "epm_test");
    }
}
