//! 出稿失败时通知运营。通道是钉钉群自定义机器人。
//!
//! `DINGTALK_WEBHOOK_URL` 为空时不发请求，出稿照常结束。
//! 正文会带上 `DINGTALK_KEYWORD`（生产环境是 `STRATEGY`），否则机器人按关键词拒收。
//! 配了 `DINGTALK_WEBHOOK_SECRET` 才按钉钉加签算法附上 `timestamp` 和 `sign`。
//! 告警失败只打 WARN，不影响出稿。
//!
//! 不把方舟密钥、用户 JWT、`access_token` 或加签密钥写进正文。原图只留对象 key。

use std::future::Future;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use chrono::{DateTime, FixedOffset, Utc};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use uuid::Uuid;

use crate::config::Config;

#[derive(Clone, Debug)]
pub struct FailureNotice {
    pub job_id: Uuid,
    pub stage: String,
    pub user_message: String,
    pub upstream: Option<String>,
    pub provider: String,
    pub board_size: i32,
    pub object_key: Option<String>,
    pub at: DateTime<Utc>,
}

pub(crate) struct Outbound {
    pub url: String,
    pub body: String,
}

/// 任务已经标成失败之后调用。未配置 webhook 时立刻返回，不建 HTTP 客户端。
pub fn spawn_failure_alert(config: Arc<Config>, notice: FailureNotice) {
    if config.dingtalk_webhook_url.trim().is_empty() {
        return;
    }
    tokio::spawn(async move {
        let now_ms = now_millis();
        emit(config.as_ref(), &notice, now_ms, post_json).await;
    });
}

pub(crate) async fn emit<F, Fut>(config: &Config, notice: &FailureNotice, now_ms: u64, post: F)
where
    F: FnOnce(String, String) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let Some(outbound) = prepare_outbound(config, notice, now_ms) else {
        return;
    };
    if config.dingtalk_keyword.trim().is_empty() {
        tracing::warn!("钉钉机器人关键词未配置，消息可能被拒收");
    }
    if let Err(err) = post(outbound.url, outbound.body).await {
        tracing::warn!(error = %sanitize_transport_error(&err), "出稿告警发送失败");
    }
}

pub(crate) fn prepare_outbound(
    config: &Config,
    notice: &FailureNotice,
    now_ms: u64,
) -> Option<Outbound> {
    let webhook = config.dingtalk_webhook_url.trim();
    if webhook.is_empty() {
        return None;
    }
    let url = signed_webhook(webhook, config.dingtalk_webhook_secret.trim(), now_ms);
    let body = message_body(config, notice);
    Some(Outbound { url, body })
}

fn message_body(config: &Config, notice: &FailureNotice) -> String {
    let secrets = secret_values(config);
    let secret_refs: Vec<&str> = secrets.iter().map(String::as_str).collect();
    let keyword = config.dingtalk_keyword.trim();
    let user_message = one_line(&redact(&notice.user_message, &secret_refs));
    let upstream = notice
        .upstream
        .as_deref()
        .map(|text| one_line(&redact(text, &secret_refs)))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "（无）".into());
    let object_key = notice
        .object_key
        .as_deref()
        .map(object_key_only)
        .filter(|key| !key.is_empty())
        .map(|key| one_line(&redact(&key, &secret_refs)))
        .filter(|key| !key.is_empty())
        .unwrap_or_else(|| "（无）".into());
    let stage = one_line(&redact(&notice.stage, &secret_refs));
    let provider = one_line(&redact(&notice.provider, &secret_refs));
    let headline = if keyword.is_empty() {
        "出稿失败".to_string()
    } else {
        format!("{keyword} 出稿失败")
    };
    let mut text = format!(
        "### {headline}\n\n- job_id: `{job}`\n- stage: {stage}\n- message: {user_message}\n- upstream: {upstream}\n- provider: {provider}\n- board_size: {board}\n- object_key: {object_key}\n- time: {time}\n",
        job = notice.job_id,
        board = notice.board_size,
        time = format_stamp(notice.at),
    );
    if !keyword.is_empty() && !text.contains(keyword) {
        text = format!("{keyword}\n{text}");
    }
    let title: String = headline.chars().take(64).collect();
    json!({
        "msgtype": "markdown",
        "markdown": {
            "title": title,
            "text": text,
        }
    })
    .to_string()
}

fn signed_webhook(webhook: &str, secret: &str, now_ms: u64) -> String {
    if secret.is_empty() {
        return webhook.to_string();
    }
    let sign = sign_query(secret, now_ms);
    let joiner = if webhook.contains('?') { '&' } else { '?' };
    format!("{webhook}{joiner}timestamp={now_ms}&sign={sign}")
}

/// 钉钉自定义机器人加签：`Base64(HmacSHA256("{timestamp}\n{secret}", secret))` 再做 URL 编码。
fn sign_query(secret: &str, now_ms: u64) -> String {
    let string_to_sign = format!("{now_ms}\n{secret}");
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac accepts this key length");
    mac.update(string_to_sign.as_bytes());
    percent_encode(&STANDARD.encode(mac.finalize().into_bytes()))
}

fn percent_encode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

async fn post_json(url: String, body: String) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "告警 HTTP 客户端初始化失败".to_string())?;
    let response = client
        .post(&url)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|err| sanitize_transport_error(&err.to_string()))?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("http {}", status.as_u16()));
    }
    if let Ok(value) = serde_json::from_str::<Value>(&text) {
        if let Some(code) = value.get("errcode").and_then(|item| item.as_i64()) {
            if code != 0 {
                let msg = value
                    .get("errmsg")
                    .and_then(|item| item.as_str())
                    .unwrap_or("");
                let msg: String = msg.chars().take(120).collect();
                return Err(sanitize_transport_error(&format!(
                    "dingtalk errcode {code} {msg}"
                )));
            }
        }
    }
    Ok(())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_millis() as u64)
        .unwrap_or(0)
}

fn format_stamp(at: DateTime<Utc>) -> String {
    let shanghai = FixedOffset::east_opt(8 * 3600).expect("Asia/Shanghai offset");
    at.with_timezone(&shanghai)
        .format("%Y-%m-%d %H:%M:%S Asia/Shanghai")
        .to_string()
}

fn secret_values(config: &Config) -> Vec<String> {
    let mut values = vec![
        config.ark_api_key.clone(),
        config.jwt_secret.clone(),
        config.wx_secret.clone(),
        config.database_url.clone(),
        config.wechat_mch_private_key.clone(),
        config.wechat_pay_api_v3_key.clone(),
        config.wechat_pay_public_key.clone(),
        config.wx_virtual_appkey_sandbox.clone(),
        config.wx_virtual_appkey.clone(),
        config.wx_virtual_appkey_prod.clone(),
        config.wx_virtual_msg_token.clone(),
        config.ops_refund_token.clone(),
        config.dingtalk_webhook_secret.clone(),
        config.dingtalk_webhook_url.clone(),
    ];
    if let Some(token) = access_token(config.dingtalk_webhook_url.trim()) {
        values.push(token);
    }
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| value.len() >= 8)
        .collect()
}

fn access_token(url: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=')?;
        if key == "access_token" {
            let decoded = percent_decode(value);
            if decoded.len() >= 8 {
                return Some(decoded);
            }
        }
    }
    None
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(&raw[index + 1..index + 3], 16) {
                out.push(value);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn redact(input: &str, secrets: &[&str]) -> String {
    let mut needles: Vec<&str> = secrets
        .iter()
        .copied()
        .map(str::trim)
        .filter(|secret| secret.len() >= 8)
        .collect();
    needles.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
    needles.dedup();
    let mut text = input.to_string();
    for secret in needles {
        if text.contains(secret) {
            text = text.replace(secret, "[redacted]");
        }
    }
    text = redact_bearer(&text);
    text = redact_jwts(&text);
    text = redact_data_urls(&text);
    text = redact_urls(&text);
    redact_query_secrets(&text)
}

fn redact_bearer(input: &str) -> String {
    replace_token(input, "Bearer ")
}

fn redact_jwts(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = rest.find("eyJ") {
        out.push_str(&rest[..idx]);
        let tail = &rest[idx..];
        if let Some(end) = jwt_len(tail) {
            out.push_str("[redacted]");
            rest = &tail[end..];
        } else {
            out.push_str("eyJ");
            rest = &tail[3..];
        }
    }
    out.push_str(rest);
    out
}

fn jwt_len(raw: &str) -> Option<usize> {
    let bytes = raw.as_bytes();
    let mut index = 0;
    let mut dots = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' {
            index += 1;
            continue;
        }
        if byte == b'.' && dots < 2 {
            dots += 1;
            index += 1;
            continue;
        }
        break;
    }
    if dots == 2 && index > 12 {
        Some(index)
    } else {
        None
    }
}

fn redact_data_urls(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = rest.find("data:image/") {
        out.push_str(&rest[..idx]);
        out.push_str("[inline-image]");
        let tail = &rest[idx..];
        let Some(comma) = tail.find(',') else {
            rest = &tail["data:image/".len()..];
            continue;
        };
        let payload = &tail[comma + 1..];
        let end = payload.find(char::is_whitespace).unwrap_or(payload.len());
        rest = &payload[end..];
    }
    out.push_str(rest);
    out
}

fn redact_urls(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = find_url(rest) {
        out.push_str(&rest[..idx]);
        let tail = &rest[idx..];
        let raw_len = tail.find(char::is_whitespace).unwrap_or(tail.len());
        let raw = &tail[..raw_len];
        let trimmed = raw.trim_end_matches(|ch: char| {
            matches!(ch, ')' | ']' | ',' | '.' | ';' | '"' | '\'' | '。' | '，')
        });
        out.push_str(&url_path_only(trimmed));
        out.push_str(&raw[trimmed.len()..]);
        rest = &tail[raw_len..];
    }
    out.push_str(rest);
    out
}

fn find_url(raw: &str) -> Option<usize> {
    match (raw.find("http://"), raw.find("https://")) {
        (Some(http), Some(https)) => Some(http.min(https)),
        (Some(http), None) => Some(http),
        (None, Some(https)) => Some(https),
        (None, None) => None,
    }
}

fn url_path_only(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let path = rest.split_once('/').map(|(_, path)| path).unwrap_or("");
    let path = path.split(['?', '#']).next().unwrap_or("");
    let path = path.trim_start_matches('/');
    path.strip_prefix("files/").unwrap_or(path).to_string()
}

pub fn object_key_only(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('`');
    let path = if find_url(trimmed).is_some() {
        let start = find_url(trimmed).unwrap_or(0);
        let tail = &trimmed[start..];
        let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
        let url = tail[..end]
            .trim_end_matches(|ch: char| matches!(ch, ')' | ']' | ',' | '.' | ';' | '"' | '\''));
        url_path_only(url)
    } else {
        trimmed
            .split(['?', '#'])
            .next()
            .unwrap_or(trimmed)
            .trim_start_matches('/')
            .to_string()
    };
    path
}

fn redact_query_secrets(input: &str) -> String {
    redact_query_param(&redact_query_param(input, "access_token"), "sign")
}

fn redact_query_param(input: &str, key: &str) -> String {
    replace_token(input, &format!("{key}="))
}

fn replace_token(input: &str, needle: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = rest.find(needle) {
        out.push_str(&rest[..idx]);
        out.push_str(needle);
        out.push_str("[redacted]");
        let tail = &rest[idx + needle.len()..];
        let end = tail
            .find(|ch: char| ch == '&' || ch.is_whitespace())
            .unwrap_or(tail.len());
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

pub fn sanitize_transport_error(err: &str) -> String {
    let cleaned = redact_query_secrets(err);
    cleaned.chars().take(300).collect()
}

fn one_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex;

    fn notice() -> FailureNotice {
        FailureNotice {
            job_id: Uuid::parse_str("143d6f5b-2a89-4214-8f23-4e78721b2229").unwrap(),
            stage: "seedream".into(),
            user_message: "出稿失败，请稍后重试。本次未扣积分。".into(),
            upstream: Some(
                "OutputImageSensitiveContentDetected.PolicyViolation: copyright restrict".into(),
            ),
            provider: "seedream".into(),
            board_size: 52,
            object_key: Some("uploads/ref.png".into()),
            at: DateTime::parse_from_rfc3339("2026-11-05T01:02:03Z")
                .unwrap()
                .with_timezone(&Utc),
        }
    }

    fn config_with(url: &str, keyword: &str, secret: &str) -> Config {
        let mut config = Config::from_env();
        config.dingtalk_webhook_url = url.into();
        config.dingtalk_keyword = keyword.into();
        config.dingtalk_webhook_secret = secret.into();
        config.ark_api_key = "sk-live-ark-key-0123456789".into();
        config
    }

    fn markdown_text(body: &str) -> String {
        let value: Value = serde_json::from_str(body).unwrap();
        assert_eq!(value["msgtype"], "markdown");
        value["markdown"]["text"].as_str().unwrap().to_string()
    }

    #[test]
    fn unconfigured_webhook_prepares_nothing() {
        let config = config_with("", "STRATEGY", "");
        assert!(prepare_outbound(&config, &notice(), 1_700_000_000_000).is_none());
    }

    #[tokio::test]
    async fn unconfigured_webhook_does_not_post() {
        let config = config_with("  ", "STRATEGY", "");
        let called = Arc::new(AtomicBool::new(false));
        let flag = called.clone();
        emit(&config, &notice(), 1_700_000_000_000, move |_url, _body| {
            flag.store(true, Ordering::SeqCst);
            async { Ok(()) }
        })
        .await;
        assert!(!called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn configured_post_includes_keyword_job_and_policy_code() {
        let config = config_with(
            "https://oapi.dingtalk.com/robot/send?access_token=unit-test-token",
            "STRATEGY",
            "",
        );
        let got: Arc<Mutex<Option<(String, String)>>> = Arc::new(Mutex::new(None));
        let slot = got.clone();
        emit(&config, &notice(), 1_700_000_000_000, move |url, body| {
            let slot = slot.clone();
            async move {
                *slot.lock().expect("lock") = Some((url, body));
                Ok(())
            }
        })
        .await;
        let (url, body) = got.lock().expect("lock").clone().expect("posted");
        assert_eq!(
            url,
            "https://oapi.dingtalk.com/robot/send?access_token=unit-test-token"
        );
        assert!(!url.contains("sign="));
        let text = markdown_text(&body);
        assert!(text.contains("STRATEGY"), "{text}");
        assert!(
            text.contains("143d6f5b-2a89-4214-8f23-4e78721b2229"),
            "{text}"
        );
        assert!(text.contains("stage: seedream"), "{text}");
        assert!(
            text.contains("OutputImageSensitiveContentDetected.PolicyViolation"),
            "{text}"
        );
        assert!(text.to_lowercase().contains("copyright"), "{text}");
        assert!(text.contains("provider: seedream"), "{text}");
        assert!(text.contains("board_size: 52"), "{text}");
        assert!(text.contains("uploads/ref.png"), "{text}");
        assert!(text.contains("2026-11-05 09:02:03 Asia/Shanghai"), "{text}");
        let message_line = text.lines().find(|line| line.contains("message:")).unwrap();
        assert!(!message_line.contains("PolicyViolation"), "{message_line}");
        assert!(
            !message_line.to_lowercase().contains("copyright"),
            "{message_line}"
        );
        assert!(!body.contains("unit-test-token"), "{body}");
        assert!(!body.contains("sk-live-ark-key-0123456789"), "{body}");
    }

    #[test]
    fn sign_query_matches_dingtalk_hmac() {
        let config = config_with(
            "https://oapi.dingtalk.com/robot/send?access_token=unit-test-token",
            "STRATEGY",
            "SEC_test_secret",
        );
        let mut sample = notice();
        sample.user_message = format!("{} sk-live-ark-key-0123456789", sample.user_message);
        sample.object_key = Some(
            "https://cdn.example.com/files/uploads/ref.png?token=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.signature"
                .into(),
        );
        sample.upstream = Some(
            "OutputImageSensitiveContentDetected.PolicyViolation: copyright restrict https://cdn.example.com/files/uploads/ref.png?access_token=supersecrettokenvalue SEC_test_secret".into(),
        );
        let outbound = prepare_outbound(&config, &sample, 1_700_000_000_000).unwrap();
        assert!(
            outbound.url.contains("timestamp=1700000000000"),
            "{}",
            outbound.url
        );
        assert!(
            outbound
                .url
                .contains("sign=Y2r6fEfRGij3p64GVF%2B%2FCNlZCaiH3LPZwPU9dtabvjU%3D"),
            "{}",
            outbound.url
        );
        assert!(outbound.url.starts_with(
            "https://oapi.dingtalk.com/robot/send?access_token=unit-test-token&timestamp="
        ));
        let text = markdown_text(&outbound.body);
        assert!(text.contains("STRATEGY"));
        assert!(text.contains("OutputImageSensitiveContentDetected.PolicyViolation"));
        assert!(text.contains("uploads/ref.png"), "{text}");
        assert!(!text.contains("https://"), "{text}");
        assert!(!text.contains("supersecrettokenvalue"), "{text}");
        assert!(!text.contains("unit-test-token"), "{text}");
        assert!(!text.contains("SEC_test_secret"), "{text}");
        assert!(!text.contains("sk-live-ark-key"), "{text}");
        assert!(!text.contains("eyJ"), "{text}");
        assert!(!outbound.body.contains("SEC_test_secret"));
    }

    #[tokio::test]
    async fn transport_error_is_swallowed() {
        let config = config_with(
            "https://oapi.dingtalk.com/robot/send?access_token=unit-test-token",
            "STRATEGY",
            "",
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        emit(&config, &notice(), 1, move |_url, _body| {
            seen.fetch_add(1, Ordering::SeqCst);
            async { Err("connection refused access_token=unit-test-token".into()) }
        })
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let cleaned = sanitize_transport_error(
            "post https://oapi.dingtalk.com/robot/send?access_token=unit-test-token&timestamp=1 failed",
        );
        assert!(!cleaned.contains("unit-test-token"), "{cleaned}");
        assert!(cleaned.contains("access_token=[redacted]"), "{cleaned}");
    }

    #[test]
    fn mock_pay_config_drops_webhook_so_tests_stay_quiet() {
        let mut config = config_with(
            "https://oapi.dingtalk.com/robot/send?access_token=unit-test-token",
            "STRATEGY",
            "SEC_test_secret",
        );
        config.force_mock_pay();
        assert!(config.dingtalk_webhook_url.is_empty());
        assert!(config.dingtalk_webhook_secret.is_empty());
        assert!(prepare_outbound(&config, &notice(), 1).is_none());
    }

    #[tokio::test]
    async fn post_json_hits_mock_http_and_reports_dingtalk_errcode() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let app = axum::Router::new()
            .route(
                "/robot/send",
                axum::routing::post(
                    |axum::extract::State(tx): axum::extract::State<
                        tokio::sync::mpsc::Sender<String>,
                    >,
                     body: axum::body::Bytes| async move {
                        let _ = tx.send(String::from_utf8_lossy(&body).into_owned()).await;
                        axum::Json(json!({"errcode": 0, "errmsg": "ok"}))
                    },
                ),
            )
            .with_state(tx);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let url = format!("http://{addr}/robot/send?access_token=unit-test-token");
        let body = json!({
            "msgtype": "markdown",
            "markdown": {
                "title": "STRATEGY 出稿失败",
                "text": "STRATEGY\njob_id: 143d6f5b-2a89-4214-8f23-4e78721b2229\nstage: seedream\nupstream: OutputImageSensitiveContentDetected.PolicyViolation"
            }
        })
        .to_string();
        let mut last = String::from("not attempted");
        for _ in 0..20 {
            match post_json(url.clone(), body.clone()).await {
                Ok(()) => {
                    last.clear();
                    break;
                }
                Err(err) => {
                    last = err;
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
            }
        }
        assert!(last.is_empty(), "{last}");
        let posted = rx.recv().await.expect("body");
        assert!(posted.contains("STRATEGY"), "{posted}");
        assert!(
            posted.contains("143d6f5b-2a89-4214-8f23-4e78721b2229"),
            "{posted}"
        );
        assert!(
            posted.contains("OutputImageSensitiveContentDetected.PolicyViolation"),
            "{posted}"
        );
        assert!(!posted.contains("unit-test-token"), "{posted}");

        let reject = axum::Router::new().route(
            "/robot/send",
            axum::routing::post(|| async {
                axum::Json(json!({"errcode": 310000, "errmsg": "keywords not in content"}))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, reject).await.unwrap();
        });
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        let err = post_json(
            format!("http://{addr}/robot/send?access_token=unit-test-token"),
            body,
        )
        .await
        .expect_err("errcode");
        assert!(err.contains("310000"), "{err}");
        assert!(!err.contains("unit-test-token"), "{err}");
    }
}
