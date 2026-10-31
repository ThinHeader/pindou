//! 出稿任务进度。小程序轮询，取消未完成的任务不扣积分。

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::error::AppResult;
use crate::extract::AuthUser;
use crate::services::pipeline;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/jobs/{id}", get(detail))
        .route("/jobs/{id}/cancel", post(cancel))
}

async fn detail(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<pipeline::JobView>> {
    Ok(Json(
        pipeline::get_for_user(&state, auth.user_id, id).await?,
    ))
}

async fn cancel(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<pipeline::JobView>> {
    Ok(Json(pipeline::cancel(&state, auth.user_id, id).await?))
}
