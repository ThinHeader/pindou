use axum::{
    extract::{Multipart, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::extract::AuthUser;
use crate::services::storage::decode_image_base64;
use crate::services::{auth, credits, drafts};
use crate::state::AppState;

use super::auth::user_dto;
use super::uploads::save_image;

#[derive(Debug, Deserialize)]
pub struct PatchMe {
    pub nickname: Option<String>,
    /// 小程序登录页用 wx.request 提交的头像。与出稿 `image_base64` 相同的标准 base64。
    pub avatar_base64: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user: super::auth::UserDto,
    pub stats: StatsDto,
    pub credits: credits::CreditsView,
    pub recent_drafts: Vec<drafts::DraftCard>,
}

#[derive(Debug, Serialize)]
pub struct StatsDto {
    pub drafts: i64,
    pub works: i64,
    pub favorites: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me", get(me).put(patch_me).patch(patch_me))
        .route("/me/profile", post(save_profile))
}

async fn me(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<MeResponse>> {
    Ok(Json(load(&state, auth.user_id).await?))
}

async fn patch_me(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<PatchMe>,
) -> AppResult<Json<MeResponse>> {
    let avatar_key = match body
        .avatar_base64
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(raw) => {
            let bytes = decode_image_base64(raw)?;
            Some(save_image(&state.storage, &bytes).await?.key)
        }
        None => None,
    };
    // 只改昵称时，缺省或空白仍报「1 到 20 个字」。带了头像时，空白昵称表示不改昵称。
    let nickname = if avatar_key.is_some() {
        body.nickname
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    } else {
        Some(body.nickname.as_deref().unwrap_or(""))
    };
    auth::save_display(&state.db, auth.user_id, nickname, avatar_key.as_deref()).await?;
    Ok(Json(load(&state, auth.user_id).await?))
}

/// 登录页一次提交头像和昵称。`file` 可选，没有新头像时只改昵称。
async fn save_profile(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<MeResponse>> {
    let mut nickname: Option<String> = None;
    let mut file: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("上传内容无法解析，请重试".into()))?
    {
        match field.name().unwrap_or("") {
            "nickname" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| AppError::BadRequest("昵称无法读取".into()))?;
                let text = text.trim().to_string();
                if !text.is_empty() {
                    nickname = Some(text);
                }
            }
            "file" | "image" => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| AppError::BadRequest("读取头像失败".into()))?;
                if !bytes.is_empty() {
                    file = Some(bytes.to_vec());
                }
            }
            _ => {}
        }
    }
    let avatar_key = match file {
        Some(bytes) => Some(save_image(&state.storage, &bytes).await?.key),
        None => None,
    };
    auth::save_display(
        &state.db,
        auth.user_id,
        nickname.as_deref(),
        avatar_key.as_deref(),
    )
    .await?;
    Ok(Json(load(&state, auth.user_id).await?))
}

async fn load(state: &AppState, user_id: Uuid) -> AppResult<MeResponse> {
    let user = auth::get_user(&state.db, user_id).await?;
    let drafts_n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM drafts WHERE user_id = ?1 AND is_official = FALSE",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    let works_n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM works WHERE user_id = ?1")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    let fav_drafts: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM draft_favorites WHERE user_id = ?1")
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
    let fav_works: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM work_favorites WHERE user_id = ?1")
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
    let recent_drafts = drafts::list(state, user_id, "mine", "").await?;
    let recent_drafts = recent_drafts.into_iter().take(6).collect();
    Ok(MeResponse {
        user: user_dto(&state.storage, &user),
        stats: StatsDto {
            drafts: drafts_n,
            works: works_n,
            favorites: fav_drafts + fav_works,
        },
        credits: credits::view(state, user_id).await?,
        recent_drafts,
    })
}
