//! 积分：查询 / 日领 / 广告 / 示意充值

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppResult;
use crate::extract::AuthUser;
use crate::services::credits::{self, CreditMutation, CreditsView, LedgerItem, OrderView, Pack};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LedgerQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateOrderRequest {
    pub pack_code: String,
}

#[derive(Debug, Serialize)]
pub struct PayResponse {
    pub order: OrderView,
    pub balance: i32,
    pub delta: i32,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/credits/me", get(me))
        .route("/credits/daily-claim", post(daily_claim))
        .route("/credits/ad-reward", post(ad_reward))
        .route("/credits/ledger", get(ledger))
        .route("/credits/packs", get(packs))
        .route("/credits/orders", post(create_order))
        .route("/credits/orders/{id}/mock-callback", post(mock_callback))
}

async fn me(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<CreditsView>> {
    Ok(Json(credits::view(&state, auth.user_id).await?))
}

async fn daily_claim(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<CreditMutation>> {
    Ok(Json(credits::daily_claim(&state, auth.user_id).await?))
}

async fn ad_reward(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<CreditMutation>> {
    Ok(Json(credits::ad_reward(&state, auth.user_id).await?))
}

async fn ledger(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<LedgerQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let items: Vec<LedgerItem> =
        credits::ledger(&state.db, auth.user_id, q.limit.unwrap_or(20)).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn packs() -> Json<serde_json::Value> {
    let items: Vec<&Pack> = credits::PACKS.iter().collect();
    Json(serde_json::json!({
        "items": items,
        "mock": true,
        "note": "示意档位，mock-callback 直接到账，不会产生真实扣款"
    }))
}

async fn create_order(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateOrderRequest>,
) -> AppResult<Json<OrderView>> {
    Ok(Json(
        credits::create_order(&state.db, auth.user_id, &body.pack_code).await?,
    ))
}

async fn mock_callback(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PayResponse>> {
    let (order, balance, delta) = credits::mock_pay(&state, auth.user_id, id).await?;
    Ok(Json(PayResponse {
        order,
        balance,
        delta,
    }))
}
