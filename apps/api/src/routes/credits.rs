//! 积分：查询 / 日领 / 广告 / 充值（开发示意或微信支付 JSAPI）

use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode};
use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::extract::AuthUser;
use crate::services::credits::{self, CreditMutation, CreditsView, LedgerItem, OrderView, Pack};
use crate::services::wechat_pay::{self, NotifyHeaders};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LedgerQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateOrderRequest {
    pub pack_code: String,
}

#[derive(Debug, serde::Serialize)]
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
        .route("/credits/orders/{id}", get(get_order))
        .route("/credits/orders/{id}/mock-callback", post(mock_callback))
        .route("/pay/wechat/notify", post(wechat_notify))
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

async fn packs(State(state): State<AppState>) -> Json<serde_json::Value> {
    let items: Vec<&Pack> = credits::PACKS.iter().collect();
    let meta = credits::pack_list_meta(&state);
    Json(serde_json::json!({
        "items": items,
        "mock": meta.mock,
        "pay_mode": meta.pay_mode,
        "note": meta.note
    }))
}

async fn create_order(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateOrderRequest>,
) -> AppResult<Json<OrderView>> {
    Ok(Json(
        credits::create_order(&state, auth.user_id, &body.pack_code).await?,
    ))
}

async fn get_order(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<OrderView>> {
    Ok(Json(credits::get_order(&state, auth.user_id, id).await?))
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

/// 微信支付结果通知。无登录态；验签失败不加积分。成功和重复通知都返回 200。
async fn wechat_notify(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(timestamp) = header_str(&headers, "wechatpay-timestamp") else {
        return notify_fail(StatusCode::UNAUTHORIZED, "缺少时间戳");
    };
    let Some(nonce) = header_str(&headers, "wechatpay-nonce") else {
        return notify_fail(StatusCode::UNAUTHORIZED, "缺少随机串");
    };
    let Some(signature) = header_str(&headers, "wechatpay-signature") else {
        return notify_fail(StatusCode::UNAUTHORIZED, "缺少签名");
    };
    let Some(serial) = header_str(&headers, "wechatpay-serial") else {
        return notify_fail(StatusCode::UNAUTHORIZED, "缺少证书序列号");
    };

    let opened = wechat_pay::open_notify(
        &state,
        NotifyHeaders {
            timestamp: &timestamp,
            nonce: &nonce,
            signature: &signature,
            serial: &serial,
        },
        &body,
    )
    .await;
    let opened = match opened {
        Ok(opened) => opened,
        Err(err) => return notify_error(err),
    };
    match credits::accept_opened_notify(&state, opened).await {
        Ok(()) => notify_ok(),
        Err(err) => notify_error(err),
    }
}

fn header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn notify_ok() -> Response {
    (
        StatusCode::OK,
        Json(json!({"code": "SUCCESS", "message": "成功"})),
    )
        .into_response()
}

fn notify_fail(status: StatusCode, message: &str) -> Response {
    tracing::warn!(%status, %message, "wechat notify rejected");
    (status, Json(json!({"code": "FAIL", "message": message}))).into_response()
}

fn notify_error(err: AppError) -> Response {
    tracing::warn!(error = %err, "wechat notify failed");
    let status = match &err {
        AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
        AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
        AppError::NotFound(_) => StatusCode::NOT_FOUND,
        AppError::Forbidden(_) => StatusCode::FORBIDDEN,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    notify_fail(status, "处理失败")
}
