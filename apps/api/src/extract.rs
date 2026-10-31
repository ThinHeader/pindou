//! 从 Authorization: Bearer 取出当前用户。

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::HeaderMap;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::services::auth;
use crate::state::AppState;

#[derive(Debug, Clone, Copy)]
pub struct AuthUser {
    pub user_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub struct MaybeUser {
    pub user_id: Option<Uuid>,
}

pub fn bearer_user(state: &AppState, headers: &HeaderMap) -> AppResult<Option<Uuid>> {
    let Some(value) = headers.get(axum::http::header::AUTHORIZATION) else {
        return Ok(None);
    };
    let raw = value
        .to_str()
        .map_err(|_| AppError::Unauthorized("登录凭证无效".into()))?;
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))
        .ok_or_else(|| AppError::Unauthorized("请使用 Bearer Token".into()))?;
    if token.trim().is_empty() {
        return Err(AppError::Unauthorized("请先登录".into()));
    }
    Ok(Some(auth::verify_token(
        &state.config.jwt_secret,
        token.trim(),
    )?))
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match bearer_user(state, &parts.headers)? {
            Some(user_id) => Ok(AuthUser { user_id }),
            None => Err(AppError::Unauthorized("请先登录".into())),
        }
    }
}

impl FromRequestParts<AppState> for MaybeUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(MaybeUser {
            user_id: bearer_user(state, &parts.headers)?,
        })
    }
}
