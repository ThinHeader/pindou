//! 广场作品流（stub）

use axum::{Json, Router, routing::get};
use serde_json::{Value, json};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/plaza/feed", get(feed))
}

async fn feed() -> Json<Value> {
    Json(json!({
        "items": [],
        "next_cursor": null,
        "message": "plaza feed stub"
    }))
}
