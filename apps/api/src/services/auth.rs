//! 微信登录与 JWT。
//!
//! 配置了 `WX_APPID` + `WX_SECRET` 时走 code2session；
//! 否则把 code 稳定哈希成开发态 openid（同一 code 始终是同一用户）。

use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: i64,
    iat: i64,
}

#[derive(Debug, Clone)]
pub struct UserRecord {
    pub id: Uuid,
    #[allow(dead_code)]
    pub openid: String,
    pub nickname: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WxSessionResp {
    openid: Option<String>,
    errcode: Option<i32>,
    errmsg: Option<String>,
}

pub fn issue_token(secret: &str, user_id: Uuid) -> AppResult<String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        iat: now.timestamp(),
        exp: (now + ChronoDuration::days(7)).timestamp(),
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!("签发登录凭证失败：{e}")))
}

pub fn verify_token(secret: &str, token: &str) -> AppResult<Uuid> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 60;
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::Unauthorized("登录已失效，请重新登录".into()))?;
    Uuid::parse_str(&data.claims.sub)
        .map_err(|_| AppError::Unauthorized("登录已失效，请重新登录".into()))
}

pub async fn login_with_code(config: &Config, pool: &PgPool, code: &str) -> AppResult<UserRecord> {
    let openid = resolve_openid(config, code).await?;
    upsert_user(pool, &openid).await
}

pub async fn resolve_openid(config: &Config, code: &str) -> AppResult<String> {
    let code = code.trim();
    if code.is_empty() {
        return Err(AppError::BadRequest("code 不能为空".into()));
    }
    if !config.wechat_configured() {
        return Ok(mock_openid(code));
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| AppError::Internal(e.into()))?;
    let resp = client
        .get("https://api.weixin.qq.com/sns/jscode2session")
        .query(&[
            ("appid", config.wx_appid.as_str()),
            ("secret", config.wx_secret.as_str()),
            ("js_code", code),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("微信登录请求失败：{e}")))?;

    let body: WxSessionResp = resp
        .json()
        .await
        .map_err(|_| AppError::BadRequest("微信登录响应无法解析，请稍后再试".into()))?;

    if let Some(openid) = body.openid.filter(|s| !s.is_empty()) {
        return Ok(openid);
    }
    let detail = body.errmsg.unwrap_or_else(|| "未知错误".into());
    let code_n = body.errcode.unwrap_or(-1);
    tracing::warn!(errcode = code_n, errmsg = %detail, "code2session failed");
    Err(AppError::Unauthorized(format!(
        "微信登录失败（{code_n}）：{detail}"
    )))
}

/// 开发态：`mock:` 前缀直接作为 openid 后缀；其余 code 做稳定哈希。
pub fn mock_openid(code: &str) -> String {
    let code = code.trim();
    if let Some(rest) = code.strip_prefix("mock:") {
        let rest = rest.trim();
        if rest.is_empty() {
            return "mock:empty".into();
        }
        return format!("mock:{rest}");
    }
    let mut hasher = Sha256::new();
    hasher.update(b"pindou-dev-openid-v1:");
    hasher.update(code.as_bytes());
    format!("dev:{}", &hex::encode(hasher.finalize())[..32])
}

pub async fn upsert_user(pool: &PgPool, openid: &str) -> AppResult<UserRecord> {
    let row = sqlx::query_as::<_, UserRow>(
        r#"
        INSERT INTO users (openid, nickname)
        VALUES ($1, '拼豆玩家')
        ON CONFLICT (openid) DO UPDATE SET updated_at = now()
        RETURNING id, openid, nickname, avatar_url
        "#,
    )
    .bind(openid)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO credit_accounts (user_id, balance)
        VALUES ($1, 0)
        ON CONFLICT (user_id) DO NOTHING
        "#,
    )
    .bind(row.id)
    .execute(pool)
    .await?;

    Ok(row.into())
}

pub async fn update_nickname(
    pool: &PgPool,
    user_id: Uuid,
    nickname: &str,
) -> AppResult<UserRecord> {
    let nickname = nickname.trim();
    let len = nickname.chars().count();
    if !(1..=20).contains(&len) {
        return Err(AppError::BadRequest("昵称需要 1 到 20 个字".into()));
    }
    let row = sqlx::query_as::<_, UserRow>(
        r#"
        UPDATE users SET nickname = $2, updated_at = now()
        WHERE id = $1
        RETURNING id, openid, nickname, avatar_url
        "#,
    )
    .bind(user_id)
    .bind(nickname)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
    Ok(row.into())
}

pub async fn get_user(pool: &PgPool, user_id: Uuid) -> AppResult<UserRecord> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, openid, nickname, avatar_url FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
    Ok(row.into())
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    openid: String,
    nickname: String,
    avatar_url: Option<String>,
}

impl From<UserRow> for UserRecord {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            openid: row.openid,
            nickname: row.nickname,
            avatar_url: row.avatar_url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_openid_is_stable() {
        assert_eq!(mock_openid("dev-device-a"), mock_openid("dev-device-a"));
        assert_ne!(mock_openid("dev-device-a"), mock_openid("dev-device-b"));
        assert_eq!(mock_openid("mock:alice"), "mock:alice");
    }
}
