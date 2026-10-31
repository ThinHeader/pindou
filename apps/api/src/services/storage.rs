//! 对象存储。
//!
//! 本地文件系统实现，响应形状对齐腾讯云 COS PutObject：
//! `key` / `url` / `etag` / `content_type`。
//! 换成 COS 时保持这四个字段即可，业务层不感知厂商。

use std::path::{Component, Path, PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::fs;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
pub struct StoredObject {
    pub key: String,
    pub url: String,
    pub etag: String,
    pub content_type: String,
}

#[derive(Clone)]
pub struct LocalStorage {
    root: PathBuf,
    public_base: String,
}

impl LocalStorage {
    pub fn new(dir: &str, public_base: &str) -> anyhow::Result<Self> {
        let root = PathBuf::from(dir);
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        Ok(Self {
            root,
            public_base: public_base.trim_end_matches('/').to_string(),
        })
    }

    pub fn public_url(&self, key: &str) -> String {
        format!("{}/files/{key}", self.public_base)
    }

    pub async fn put_object(
        &self,
        key: &str,
        bytes: &[u8],
        content_type: &str,
    ) -> AppResult<StoredObject> {
        let path = self.resolve_writable(key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("创建存储目录失败：{e}")))?;
        }
        fs::write(&path, bytes)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("写入对象失败：{e}")))?;

        let etag = etag_of(bytes);
        Ok(StoredObject {
            key: key.to_string(),
            url: self.public_url(key),
            etag,
            content_type: content_type.to_string(),
        })
    }

    pub async fn get_object(&self, key: &str) -> AppResult<(Vec<u8>, String)> {
        let path = self.resolve_existing(key)?;
        let bytes = fs::read(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::NotFound("找不到参考图，请重新上传".into())
            } else {
                AppError::Internal(anyhow::anyhow!("读取对象失败：{e}"))
            }
        })?;
        Ok((bytes, content_type_for(key).to_string()))
    }

    pub async fn delete_object(&self, key: &str) -> AppResult<()> {
        if let Ok(path) = self.resolve_existing(key) {
            let _ = fs::remove_file(path).await;
        }
        Ok(())
    }

    fn resolve_writable(&self, key: &str) -> AppResult<PathBuf> {
        let rel = safe_relative(key)?;
        Ok(self.root.join(rel))
    }

    fn resolve_existing(&self, key: &str) -> AppResult<PathBuf> {
        let path = self.resolve_writable(key)?;
        let root = &self.root;
        // 拒绝逃出根目录（key 已经过滤了 ..，这里再确认拼接结果）
        if !path.starts_with(root) {
            return Err(AppError::BadRequest("非法对象键".into()));
        }
        Ok(path)
    }
}

pub fn content_type_for(key: &str) -> &'static str {
    let lower = key.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "application/octet-stream"
    }
}

pub fn etag_of(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(&digest[..8])
}

fn safe_relative(key: &str) -> AppResult<PathBuf> {
    if key.is_empty()
        || key.contains('\\')
        || key.starts_with('/')
        || key
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(AppError::BadRequest("非法对象键".into()));
    }
    let path = Path::new(key);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(AppError::BadRequest("非法对象键".into()));
    }
    Ok(path.to_path_buf())
}

/// 去掉 data URL 前缀后做标准 base64 解码。
pub fn decode_image_base64(input: &str) -> AppResult<Vec<u8>> {
    let s = input.trim();
    if s.is_empty() {
        return Err(AppError::BadRequest("图片数据为空".into()));
    }
    let payload = if let Some(rest) = s.strip_prefix("data:") {
        rest.split_once(',')
            .map(|(_, data)| data)
            .ok_or_else(|| AppError::BadRequest("图片数据无效，请重新选择 JPG 或 PNG".into()))?
    } else {
        s
    };
    let payload: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    STANDARD
        .decode(payload)
        .map_err(|_| AppError::BadRequest("图片数据无效，请重新选择 JPG 或 PNG".into()))
}

/// 把客户端传来的 image_ref（对象键、`/files/...` 或绝对 URL）收成存储键。
pub fn normalize_object_key(refer: &str, public_base: &str) -> String {
    let refer = refer.trim();
    let base = public_base.trim_end_matches('/');
    if let Some(rest) = refer.strip_prefix(base) {
        return strip_files_prefix(rest);
    }
    if let Some(idx) = refer.find("/files/") {
        return refer[idx + "/files/".len()..].to_string();
    }
    strip_files_prefix(refer)
}

fn strip_files_prefix(s: &str) -> String {
    s.trim_start_matches('/')
        .trim_start_matches("files/")
        .to_string()
}
