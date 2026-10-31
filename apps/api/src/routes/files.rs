use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
};

use crate::error::AppError;
use crate::services::storage::content_type_for;
use crate::state::AppState;

pub async fn serve(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Result<Response, AppError> {
    let (bytes, content_type) = state.storage.get_object(&path).await?;
    let content_type = if content_type == "application/octet-stream" {
        content_type_for(&path).to_string()
    } else {
        content_type
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(bytes))
        .map_err(|e| AppError::Internal(e.into()))
}
