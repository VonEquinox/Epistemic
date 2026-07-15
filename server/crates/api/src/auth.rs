use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use epistemic_core::domain::{UserPublic, UserRole};
use epistemic_core::AppError;
use tower_sessions::Session;
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

pub const SESSION_USER_KEY: &str = "user_id";

pub struct AuthUser(pub UserPublic);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError(AppError::Unauthorized))?;

        let user_id: Uuid = session
            .get(SESSION_USER_KEY)
            .await
            .map_err(|e| ApiError(AppError::Other(anyhow::anyhow!(e))))?
            .ok_or(ApiError(AppError::Unauthorized))?;

        let user = epistemic_core::repo::users::find_by_id(&state.pool, user_id)
            .await?
            .ok_or(ApiError(AppError::Unauthorized))?;

        Ok(AuthUser(user))
    }
}

pub struct AdminUser(pub UserPublic);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(user) = AuthUser::from_request_parts(parts, state).await?;
        if user.role != UserRole::Admin {
            return Err(ApiError(AppError::Forbidden("admin only".into())));
        }
        Ok(AdminUser(user))
    }
}
