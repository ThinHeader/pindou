//! 作品浏览与互动已关闭。
//!
//! 个人主体不提供用户内容发布、点赞、收藏和评论。
//! 路由保留，避免旧地址落到无关处理，统一返回 410。

use axum::{
    routing::{get, post},
    Router,
};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

async fn gone() -> AppResult<()> {
    Err(AppError::Gone("该功能已关闭".into()))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/plaza/feed", get(gone))
        .route("/plaza/works/{id}", get(gone))
        .route("/plaza/works/{id}/like", post(gone))
        .route("/plaza/works/{id}/favorite", post(gone))
        .route("/plaza/works/{id}/comments", get(gone).post(gone))
}
