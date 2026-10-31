//! HTTP 路由聚合

pub mod auth;
pub mod credits;
pub mod drafts;
pub mod files;
pub mod health;
pub mod me;
pub mod plaza;
pub mod uploads;

use axum::Router;

use crate::state::AppState;

pub fn api_v1() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(me::router())
        .merge(credits::router())
        .merge(uploads::router())
        .merge(drafts::router())
        .merge(plaza::router())
}
