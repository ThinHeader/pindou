//! 客诉 / 运维接口。不对小程序用户开放，也不能用登录 JWT 调用。

use axum::http::HeaderMap;
use axum::{extract::Path, extract::State, routing::post, Json, Router};

use crate::error::{AppError, AppResult};
use crate::services::credits::{self, OpsRefund};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/ops/credit-orders/{key}/refund", post(refund_order))
}

async fn refund_order(
    State(state): State<AppState>,
    Path(key): Path<String>,
    headers: HeaderMap,
) -> AppResult<Json<OpsRefund>> {
    authorize(&state, &headers)?;
    if key.len() > 64 {
        return Err(AppError::BadRequest("订单号无效".into()));
    }
    Ok(Json(credits::ops_refund_virtual(&state, &key).await?))
}

fn authorize(state: &AppState, headers: &HeaderMap) -> AppResult<()> {
    let expected = state.config.ops_refund_token.trim();
    if expected.is_empty() {
        return Err(AppError::ServiceUnavailable(
            "未配置客诉退款口令 OPS_REFUND_TOKEN".into(),
        ));
    }
    let given = headers
        .get("x-ops-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .trim();
    if !secrets_match(expected, given) {
        return Err(AppError::Unauthorized("退款口令无效".into()));
    }
    Ok(())
}

fn secrets_match(expected: &str, given: &str) -> bool {
    let left = expected.as_bytes();
    let right = given.as_bytes();
    let mut diff = left.len() ^ right.len();
    for i in 0..left.len().max(right.len()) {
        let a = left.get(i).copied().unwrap_or(0);
        let b = right.get(i).copied().unwrap_or(0);
        diff |= (a ^ b) as usize;
    }
    diff == 0
}
