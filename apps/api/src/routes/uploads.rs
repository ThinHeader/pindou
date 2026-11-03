//! 上传参考图。本地盘保存，返回 COS 风格的 key / url / etag。

use axum::{
    extract::{Multipart, State},
    routing::post,
    Json, Router,
};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::extract::AuthUser;
use crate::services::storage::StoredObject;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/uploads", post(upload))
}

async fn upload(
    State(state): State<AppState>,
    _auth: AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<StoredObject>> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("上传内容无法解析，请重试".into()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" && name != "image" {
            continue;
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|_| AppError::BadRequest("读取上传文件失败".into()))?;
        return Ok(Json(save_image(&state.storage, &bytes).await?));
    }
    Err(AppError::BadRequest("请使用 file 字段上传图片".into()))
}

/// 把一张图片写入现有对象存储。出稿参考图和登录头像共用这一处。
pub async fn save_image(
    storage: &crate::services::storage::LocalStorage,
    bytes: &[u8],
) -> AppResult<StoredObject> {
    if bytes.is_empty() {
        return Err(AppError::BadRequest("请先选择图片".into()));
    }
    let format = image::guess_format(bytes)
        .map_err(|_| AppError::BadRequest("无法解析图片，请上传 JPG 或 PNG".into()))?;
    image::load_from_memory(bytes)
        .map_err(|_| AppError::BadRequest("无法解析图片，请上传 JPG 或 PNG".into()))?;
    let (ext, content_type) = match format {
        image::ImageFormat::Jpeg => ("jpg", "image/jpeg"),
        image::ImageFormat::Png => ("png", "image/png"),
        image::ImageFormat::Gif => ("gif", "image/gif"),
        _ => {
            return Err(AppError::BadRequest(
                "无法解析图片，请上传 JPG 或 PNG".into(),
            ))
        }
    };
    let key = format!("uploads/{}.{}", Uuid::new_v4(), ext);
    storage.put_object(&key, bytes, content_type).await
}
