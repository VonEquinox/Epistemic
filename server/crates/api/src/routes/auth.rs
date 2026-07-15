use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use epistemic_core::domain::{Invite, UserPublic, UserRole};
use epistemic_core::repo::users;
use epistemic_core::AppError;
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use utoipa::ToSchema;

use crate::auth::{AdminUser, AuthUser, SESSION_USER_KEY};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .route("/register", post(register))
        .route("/invites", post(create_invite))
        .route("/users", get(list_users))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginReq {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResp {
    pub user: UserPublic,
}

async fn login(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<LoginReq>,
) -> ApiResult<Json<LoginResp>> {
    let user = users::authenticate(&state.pool, &body.email, &body.password).await?;
    session
        .insert(SESSION_USER_KEY, user.id)
        .await
        .map_err(|e| ApiError(AppError::Other(anyhow::anyhow!(e))))?;
    Ok(Json(LoginResp { user }))
}

async fn logout(session: Session) -> ApiResult<Json<serde_json::Value>> {
    session
        .flush()
        .await
        .map_err(|e| ApiError(AppError::Other(anyhow::anyhow!(e))))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn me(AuthUser(user): AuthUser) -> ApiResult<Json<UserPublic>> {
    Ok(Json(user))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterReq {
    pub token: String,
    pub name: String,
    pub password: String,
}

async fn register(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<RegisterReq>,
) -> ApiResult<Json<LoginResp>> {
    let invite = users::find_invite_by_token(&state.pool, &body.token)
        .await?
        .ok_or_else(|| AppError::BadRequest("invalid invite token".into()))?;
    if invite.used_at.is_some() {
        return Err(AppError::BadRequest("invite already used".into()).into());
    }
    if body.password.len() < 8 {
        return Err(AppError::Validation("password must be ≥ 8 chars".into()).into());
    }
    let user = users::create_user(
        &state.pool,
        &invite.email,
        &body.name,
        &body.password,
        UserRole::Member,
    )
    .await?;
    users::mark_invite_used(&state.pool, invite.id).await?;
    session
        .insert(SESSION_USER_KEY, user.id)
        .await
        .map_err(|e| ApiError(AppError::Other(anyhow::anyhow!(e))))?;
    Ok(Json(LoginResp { user }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct InviteReq {
    pub email: String,
}

async fn create_invite(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Json(body): Json<InviteReq>,
) -> ApiResult<Json<Invite>> {
    let invite = users::create_invite(&state.pool, &body.email, admin.id).await?;
    Ok(Json(invite))
}

async fn list_users(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Vec<UserPublic>>> {
    Ok(Json(users::list_users(&state.pool).await?))
}
