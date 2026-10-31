//! 广场作品流与互动

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::AppResult;
use crate::extract::{AuthUser, MaybeUser};
use crate::services::plaza;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct FeedQuery {
    pub tab: Option<String>,
    pub q: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CommentRequest {
    pub content: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/plaza/feed", get(feed))
        .route("/plaza/works/{id}", get(work))
        .route("/plaza/works/{id}/like", post(like))
        .route("/plaza/works/{id}/favorite", post(favorite))
        .route(
            "/plaza/works/{id}/comments",
            get(comments).post(add_comment),
        )
}

async fn feed(
    State(state): State<AppState>,
    viewer: MaybeUser,
    Query(q): Query<FeedQuery>,
) -> AppResult<Json<plaza::FeedPage>> {
    Ok(Json(
        plaza::feed(
            &state,
            viewer.user_id,
            q.tab.as_deref().unwrap_or("recommend"),
            q.q.as_deref().unwrap_or(""),
            q.cursor.as_deref(),
            q.limit.unwrap_or(20),
        )
        .await?,
    ))
}

async fn work(
    State(state): State<AppState>,
    viewer: MaybeUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<plaza::WorkCard>> {
    Ok(Json(plaza::get_work(&state, viewer.user_id, id).await?))
}

async fn like(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let (liked, like_count) = plaza::toggle_like(&state, auth.user_id, id).await?;
    Ok(Json(json!({ "liked": liked, "like_count": like_count })))
}

async fn favorite(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let (favorited, favorite_count) = plaza::toggle_favorite(&state, auth.user_id, id).await?;
    Ok(Json(
        json!({ "favorited": favorited, "favorite_count": favorite_count }),
    ))
}

async fn comments(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let items = plaza::list_comments(&state, id).await?;
    Ok(Json(json!({ "items": items })))
}

async fn add_comment(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CommentRequest>,
) -> AppResult<Json<plaza::CommentDto>> {
    Ok(Json(
        plaza::add_comment(&state, auth.user_id, id, &body.content).await?,
    ))
}
