//! 微信登录（stub：无真实 code2session）

use axum::{Json, Router, extract::State, routing::post};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::{AppState, services};

#[derive(Debug, Deserialize)]
pub struct WxLoginRequest {
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct UserDto {
    pub id: String,
    pub nickname: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WxLoginResponse {
    pub token: String,
    pub user: UserDto,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/auth/wx-login", post(wx_login))
}

async fn wx_login(
    State(state): State<AppState>,
    Json(body): Json<WxLoginRequest>,
) -> AppResult<Json<WxLoginResponse>> {
    if body.code.trim().is_empty() {
        return Err(AppError::BadRequest("code is required".into()));
    }

    // 占位：未配置 WX_APPID 时返回演示用户 + 本地 JWT stub
    let user_id = Uuid::new_v4().to_string();
    let token = services::auth::issue_dev_token(&state.config.jwt_secret, &user_id)?;

    Ok(Json(WxLoginResponse {
        token,
        user: UserDto {
            id: user_id,
            nickname: "拼豆玩家".into(),
            avatar_url: None,
        },
    }))
}
