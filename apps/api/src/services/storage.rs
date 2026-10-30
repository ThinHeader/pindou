//! 对象存储占位（腾讯云 COS / 阿里云 OSS）
//!
//! 后续实现：上传原图、色号稿缩略图、作品图。
//! 当前仅定义接口形状，不连真实云。

#![allow(dead_code)]

use anyhow::Result;

#[derive(Debug, Clone)]
pub enum StorageProvider {
    Cos,
    Oss,
    LocalDev,
}

pub struct ObjectKey(pub String);

pub trait ObjectStorage: Send + Sync {
    fn put_bytes(&self, key: &ObjectKey, bytes: &[u8], content_type: &str) -> Result<String>;
    fn public_url(&self, key: &ObjectKey) -> String;
}

/// 本地开发 stub：假装上传成功，返回伪 URL
pub struct DevStorage;

impl ObjectStorage for DevStorage {
    fn put_bytes(&self, key: &ObjectKey, _bytes: &[u8], _content_type: &str) -> Result<String> {
        Ok(self.public_url(key))
    }

    fn public_url(&self, key: &ObjectKey) -> String {
        format!("https://cdn.example.local/{}", key.0)
    }
}
