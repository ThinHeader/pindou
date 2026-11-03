//! 统一错误类型 → HTTP 响应

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    Unauthorized(String),

    #[error("{0}")]
    Forbidden(String),

    #[error("{message}")]
    Conflict { code: &'static str, message: String },

    #[error("{0}")]
    InsufficientCredits(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Gone(String),

    #[error("{0}")]
    ServiceUnavailable(String),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::Conflict {
            code,
            message: message.into(),
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        tracing::error!(error = %err, "database error");
        Self::Internal(err.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, msg) = match &self {
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, "bad_request", m.clone()),
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, "unauthorized", m.clone()),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, "forbidden", m.clone()),
            AppError::Conflict { code, message } => (StatusCode::CONFLICT, *code, message.clone()),
            AppError::InsufficientCredits(m) => (
                StatusCode::PAYMENT_REQUIRED,
                "insufficient_credits",
                m.clone(),
            ),
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, "not_found", m.clone()),
            AppError::Gone(m) => (StatusCode::GONE, "gone", m.clone()),
            AppError::ServiceUnavailable(m) => {
                (StatusCode::SERVICE_UNAVAILABLE, "unavailable", m.clone())
            }
            AppError::Internal(e) => {
                tracing::error!(error = %e, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "服务器开小差了，请稍后再试".into(),
                )
            }
        };

        (status, Json(json!({ "error": code, "message": msg }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
