//! 出稿：校验积分、扣 1、返回 job stub

use axum::{Json, Router, extract::State, routing::post};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::{AppState, services};

#[derive(Debug, Deserialize)]
pub struct GenerateRequest {
    /// 图片对象键或 data URL 占位
    pub image_ref: Option<String>,
    /// 珠板尺寸，如 "52x52"
    pub board_size: Option<String>,
    /// 色数
    pub color_count: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct GenerateResponse {
    pub job_id: String,
    pub status: String,
    pub balance: i32,
    pub message: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/drafts/generate", post(generate))
}

async fn generate(
    State(state): State<AppState>,
    Json(body): Json<GenerateRequest>,
) -> AppResult<Json<GenerateResponse>> {
    let mut bal = state.demo_credits.lock().await;
    match services::credits::try_spend_generate(*bal) {
        Ok(next) => {
            *bal = next;
            let job_id = Uuid::new_v4().to_string();
            let board = body.board_size.as_deref().unwrap_or("52x52");
            let colors = body.color_count.unwrap_or(24);
            let _image = body.image_ref.as_deref();
            Ok(Json(GenerateResponse {
                job_id,
                status: "queued".into(),
                balance: *bal,
                message: format!(
                    "出稿任务已入队（stub，{board} / {colors} 色，尚未接真实渲染）"
                ),
            }))
        }
        Err(msg) => Err(AppError::InsufficientCredits(msg)),
    }
}
