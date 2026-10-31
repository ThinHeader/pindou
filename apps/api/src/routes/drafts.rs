//! 出稿与稿件库

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::AppResult;
use crate::extract::AuthUser;
use crate::services::{drafts, plaza};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum FlexBoard {
    Num(u32),
    Label(String),
}

#[derive(Debug, Deserialize)]
pub struct GenerateRequest {
    pub image_ref: Option<String>,
    pub image_base64: Option<String>,
    /// 52 / 78 / 104，或 "52x52"
    pub board_size: Option<FlexBoard>,
    pub color_count: Option<u32>,
    pub palette_pref: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub tab: Option<String>,
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PublishRequest {
    pub title: Option<String>,
    pub caption: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RenameRequest {
    pub title: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/drafts/generate", post(generate))
        .route("/drafts", get(list))
        .route(
            "/drafts/{id}",
            get(detail).put(rename).patch(rename).delete(delete_draft),
        )
        .route("/drafts/{id}/favorite", post(favorite))
        .route("/drafts/{id}/publish", post(publish))
}

async fn generate(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<GenerateRequest>,
) -> AppResult<Json<drafts::GenerateOutput>> {
    let (board_size, board_label) = match body.board_size {
        Some(FlexBoard::Num(n)) => (Some(n), None),
        Some(FlexBoard::Label(label)) => (None, Some(label)),
        None => (None, None),
    };
    let out = drafts::generate(
        &state,
        auth.user_id,
        drafts::GenerateInput {
            image_base64: body.image_base64,
            image_ref: body.image_ref,
            board_size,
            board_label,
            color_count: body.color_count,
            palette_pref: body.palette_pref,
            title: body.title,
        },
    )
    .await?;
    Ok(Json(out))
}

async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let items = drafts::list(
        &state,
        auth.user_id,
        q.tab.as_deref().unwrap_or("all"),
        q.q.as_deref().unwrap_or(""),
    )
    .await?;
    Ok(Json(json!({ "items": items, "next_cursor": null })))
}

async fn detail(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<drafts::DraftDetail>> {
    Ok(Json(drafts::get_detail(&state, auth.user_id, id).await?))
}

async fn rename(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<RenameRequest>,
) -> AppResult<Json<drafts::DraftDetail>> {
    let title = body.title.unwrap_or_default();
    Ok(Json(
        drafts::rename(&state, auth.user_id, id, &title).await?,
    ))
}

async fn delete_draft(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<drafts::DeleteOutput>> {
    Ok(Json(drafts::delete_owned(&state, auth.user_id, id).await?))
}

async fn favorite(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let favorited = drafts::toggle_favorite(&state.db, auth.user_id, id).await?;
    Ok(Json(json!({ "favorited": favorited })))
}

async fn publish(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PublishRequest>,
) -> AppResult<Json<plaza::WorkCard>> {
    Ok(Json(
        plaza::publish(
            &state,
            auth.user_id,
            id,
            body.title,
            body.caption,
            body.category,
        )
        .await?,
    ))
}
