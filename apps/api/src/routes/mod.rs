//! HTTP 路由聚合

mod auth;
mod credits;
mod drafts;
mod health;
mod plaza;

use axum::Router;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .nest("/api/v1", api_v1())
}

fn api_v1() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(credits::router())
        .merge(drafts::router())
        .merge(plaza::router())
}
