use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppResult;
use crate::extract::AuthUser;
use crate::services::{auth, credits, drafts};
use crate::state::AppState;

use super::auth::user_dto;

#[derive(Debug, Deserialize)]
pub struct PatchMe {
    pub nickname: Option<String>,
    pub avatar_key: Option<String>,
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
    Router::new().route("/me", get(me).put(patch_me).patch(patch_me))
}

async fn me(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<MeResponse>> {
    Ok(Json(load(&state, auth.user_id).await?))
}

async fn patch_me(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<PatchMe>,
) -> AppResult<Json<MeResponse>> {
    auth::update_profile(
        &state.db,
        &state.storage,
        auth.user_id,
        body.nickname.as_deref(),
        body.avatar_key.as_deref(),
    )
    .await?;
    Ok(Json(load(&state, auth.user_id).await?))
}

async fn load(state: &AppState, user_id: Uuid) -> AppResult<MeResponse> {
    let user = auth::get_user(&state.db, user_id).await?;
    let drafts_n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM drafts WHERE user_id = $1 AND is_official = FALSE",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    let works_n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM works WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    let fav_drafts: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM draft_favorites WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
    let fav_works: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM work_favorites WHERE user_id = $1")
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
