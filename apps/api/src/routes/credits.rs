//! 积分：查询 / 日领 / 广告奖励

use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use serde::Serialize;

use crate::error::AppResult;
use crate::{AppState, services};

#[derive(Debug, Serialize)]
pub struct CreditsMeResponse {
    pub balance: i32,
    pub daily_grant: i32,
    pub generate_cost: i32,
    pub ad_reward: i32,
    pub daily_claimed_today: bool,
}

#[derive(Debug, Serialize)]
pub struct CreditsMutationResponse {
    pub balance: i32,
    pub delta: i32,
    pub reason: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/credits/me", get(me))
        .route("/credits/daily-claim", post(daily_claim))
        .route("/credits/ad-reward", post(ad_reward))
}

async fn me(State(state): State<AppState>) -> AppResult<Json<CreditsMeResponse>> {
    let balance = *state.demo_credits.lock().await;
    Ok(Json(CreditsMeResponse {
        balance,
        daily_grant: services::credits::DAILY_GRANT,
        generate_cost: services::credits::GENERATE_COST,
        ad_reward: services::credits::AD_REWARD,
        daily_claimed_today: false,
    }))
}

async fn daily_claim(
    State(state): State<AppState>,
) -> AppResult<Json<CreditsMutationResponse>> {
    let mut bal = state.demo_credits.lock().await;
    *bal = services::credits::apply_daily_claim(*bal);
    Ok(Json(CreditsMutationResponse {
        balance: *bal,
        delta: services::credits::DAILY_GRANT,
        reason: "daily_claim".into(),
    }))
}

async fn ad_reward(State(state): State<AppState>) -> AppResult<Json<CreditsMutationResponse>> {
    let mut bal = state.demo_credits.lock().await;
    *bal = services::credits::apply_ad_reward(*bal);
    Ok(Json(CreditsMutationResponse {
        balance: *bal,
        delta: services::credits::AD_REWARD,
        reason: "ad_reward".into(),
    }))
}
