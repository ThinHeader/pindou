//! 微信登录。未配置 AppId 时，任意非空 code 走开发态（同一 code 同一用户）。

use axum::{extract::State, routing::post, Json, Router};
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::services::storage::LocalStorage;
use crate::services::{self, auth};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct WxLoginRequest {
    pub code: String,
}

#[derive(Debug, serde::Serialize)]
pub struct UserDto {
    pub id: String,
    pub nickname: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct WxLoginResponse {
    pub token: String,
    pub user: UserDto,
    /// dev：未配置微信，按 code 稳定映射；wechat：已走 code2session
    pub mode: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/auth/wx-login", post(wx_login))
}

pub fn user_dto(storage: &LocalStorage, user: &auth::UserRecord) -> UserDto {
    UserDto {
        id: user.id.to_string(),
        nickname: user.nickname.clone(),
        avatar_url: user
            .avatar_url
            .as_deref()
            .map(|stored| auth::avatar_public_url(storage, stored)),
    }
}

async fn wx_login(
    State(state): State<AppState>,
    Json(body): Json<WxLoginRequest>,
) -> AppResult<Json<WxLoginResponse>> {
    if body.code.trim().is_empty() {
        return Err(AppError::BadRequest("code 不能为空".into()));
    }
    let mode = if state.config.wechat_configured() {
        "wechat"
    } else {
        "dev"
    };
    let user = services::auth::login_with_code(&state.config, &state.db, &body.code).await?;
    let token = services::auth::issue_token(&state.config.jwt_secret, user.id)?;
    Ok(Json(WxLoginResponse {
        token,
        user: user_dto(&state.storage, &user),
        mode: mode.into(),
    }))
}
