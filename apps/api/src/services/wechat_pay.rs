//! 微信支付 API v3：JSAPI 预下单、小程序调起签名、回调验签与 AEAD 解密。
//!
//! 商户参数未配置时不走这里。配置齐全后，积分只在验签成功的支付结果里入账。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{DateTime, Utc};
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1v15::{Signature, SigningKey, VerifyingKey};
use rsa::pkcs8::DecodePrivateKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::signature::{SignatureEncoding, Signer, Verifier};
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

const OFFICIAL_API: &str = "https://api.mch.weixin.qq.com";
const TIMESTAMP_WINDOW_SECS: i64 = 300;

#[derive(Clone)]
pub struct PayCredentials {
    pub app_id: String,
    pub mch_id: String,
    pub serial_no: String,
    pub private_key_pem: String,
    pub api_v3_key: String,
    pub notify_url: String,
    pub public_key_pem: String,
    pub public_key_id: String,
    pub api_base: String,
}

pub enum PayMode {
    /// 没有任何商户参数。允许开发示意到账。
    Mock,
    /// 写了部分商户参数，但不能安全地请求微信，也不能示意到账。
    Misconfigured(String),
    Live(PayCredentials),
}

#[derive(Clone, Default)]
pub struct PlatformCertStore {
    inner: Arc<RwLock<CertCache>>,
}

#[derive(Default)]
struct CertCache {
    keys: HashMap<String, RsaPublicKey>,
    fetched_at: Option<Instant>,
}

impl PlatformCertStore {
    async fn get(&self, serial: &str) -> Option<RsaPublicKey> {
        self.inner.read().await.keys.get(serial).cloned()
    }

    async fn recently_fetched(&self) -> bool {
        self.inner
            .read()
            .await
            .fetched_at
            .is_some_and(|at| at.elapsed() < Duration::from_secs(600))
    }

    async fn mark_fetched(&self) {
        self.inner.write().await.fetched_at = Some(Instant::now());
    }

    async fn store(&self, keys: HashMap<String, RsaPublicKey>) {
        let mut guard = self.inner.write().await;
        if !keys.is_empty() {
            guard.keys.extend(keys);
        }
        guard.fetched_at = Some(Instant::now());
    }
}

#[derive(Debug)]
pub struct VerifiedTransaction {
    pub out_trade_no: String,
    pub transaction_id: String,
    pub appid: String,
    pub mchid: String,
    pub payer_openid: String,
    pub amount_total: i64,
    pub trade_state: String,
}

pub enum OpenedNotify {
    /// 验签通过且交易成功，可以入账（入账本身仍要幂等）。
    Paid(VerifiedTransaction),
    /// 验签通过，但不是支付成功，不要入账。
    Ignored,
}

pub struct NotifyHeaders<'a> {
    pub timestamp: &'a str,
    pub nonce: &'a str,
    pub signature: &'a str,
    pub serial: &'a str,
}

#[derive(Debug, Serialize)]
pub struct JsapiPayParams {
    #[serde(rename = "timeStamp")]
    pub time_stamp: String,
    #[serde(rename = "nonceStr")]
    pub nonce_str: String,
    pub package: String,
    #[serde(rename = "signType")]
    pub sign_type: String,
    #[serde(rename = "paySign")]
    pub pay_sign: String,
}

pub fn pay_mode(config: &Config) -> PayMode {
    if !config.pay_credentials_present {
        return PayMode::Mock;
    }
    if let Some(err) = &config.pay_config_error {
        return PayMode::Misconfigured(err.clone());
    }

    let mut missing = Vec::new();
    let app_id = config.mini_app_id().to_string();
    if app_id.is_empty() {
        missing.push("WECHAT_APP_ID 或 WX_APPID");
    }
    if config.wechat_mch_id.trim().is_empty() {
        missing.push("WECHAT_MCH_ID");
    }
    if config.wechat_mch_serial_no.trim().is_empty() {
        missing.push("WECHAT_MCH_SERIAL_NO");
    }
    if config.wechat_mch_private_key.trim().is_empty() {
        missing.push("WECHAT_MCH_PRIVATE_KEY");
    }
    if config.wechat_pay_api_v3_key.len() != 32 {
        missing.push("WECHAT_PAY_API_V3_KEY（32 位）");
    }
    let has_pub = !config.wechat_pay_public_key.trim().is_empty();
    let has_pub_id = !config.wechat_pay_public_key_id.trim().is_empty();
    if has_pub != has_pub_id {
        missing.push("WECHAT_PAY_PUBLIC_KEY 与 WECHAT_PAY_PUBLIC_KEY_ID");
    }
    if !missing.is_empty() {
        return PayMode::Misconfigured(format!("缺少 {}", missing.join("、")));
    }

    let notify = resolved_notify_url(config);
    if notify.is_empty() {
        return PayMode::Misconfigured("缺少 WECHAT_PAY_NOTIFY_URL".into());
    }
    if notify.contains('?') {
        return PayMode::Misconfigured("WECHAT_PAY_NOTIFY_URL 不能带查询参数".into());
    }
    let api_base = normalized_api_base(config);
    if official_api(&api_base) && !notify.starts_with("https://") {
        return PayMode::Misconfigured(
            "回调地址必须是微信服务器能访问的 https URL，请设置 WECHAT_PAY_NOTIFY_URL".into(),
        );
    }
    if let Err(err) = parse_private_key(&config.wechat_mch_private_key) {
        return PayMode::Misconfigured(format!("商户私钥无法解析：{err}"));
    }
    if has_pub {
        if let Err(err) = parse_public_key(&config.wechat_pay_public_key) {
            return PayMode::Misconfigured(format!("微信支付公钥无法解析：{err}"));
        }
    }

    PayMode::Live(PayCredentials {
        app_id,
        mch_id: config.wechat_mch_id.trim().to_string(),
        serial_no: config.wechat_mch_serial_no.trim().to_string(),
        private_key_pem: config.wechat_mch_private_key.clone(),
        api_v3_key: config.wechat_pay_api_v3_key.clone(),
        notify_url: notify,
        public_key_pem: config.wechat_pay_public_key.clone(),
        public_key_id: config.wechat_pay_public_key_id.trim().to_string(),
        api_base,
    })
}

pub fn describe_pay_mode(config: &Config) -> String {
    match pay_mode(config) {
        PayMode::Mock => "微信支付未配置，充值走开发示意（mock-callback）".into(),
        PayMode::Misconfigured(reason) => format!("微信支付配置不完整，示意到账已关闭：{reason}"),
        PayMode::Live(_) => "微信支付 JSAPI 已启用".into(),
    }
}

pub async fn jsapi_prepay(
    state: &AppState,
    creds: &PayCredentials,
    description: &str,
    out_trade_no: &str,
    amount_fen: i32,
    openid: &str,
) -> AppResult<String> {
    if !valid_out_trade_no(out_trade_no) {
        return Err(AppError::BadRequest("商户订单号无效".into()));
    }
    if openid.trim().is_empty() {
        return Err(AppError::BadRequest("缺少付款人 openid".into()));
    }
    let payload = serde_json::json!({
        "appid": creds.app_id,
        "mchid": creds.mch_id,
        "description": truncate_bytes(description, 127),
        "out_trade_no": out_trade_no,
        "notify_url": creds.notify_url,
        "amount": { "total": amount_fen, "currency": "CNY" },
        "payer": { "openid": openid },
    });
    let body = serde_json::to_string(&payload).map_err(|err| AppError::Internal(err.into()))?;
    let resp = signed_request(creds, "POST", "/v3/pay/transactions/jsapi", &body).await?;
    if should_verify_response(&creds.api_base) {
        verify_http_signature(state, creds, &resp).await?;
    }
    if !resp.status.is_success() {
        return Err(AppError::BadRequest(format!(
            "微信支付下单失败：{}",
            wechat_error_message(&resp.body)
        )));
    }
    let parsed: PrepayResponse = serde_json::from_slice(&resp.body)
        .map_err(|_| AppError::BadRequest("微信支付下单响应无法解析".into()))?;
    if parsed.prepay_id.is_empty() || parsed.prepay_id.chars().any(|c| c.is_whitespace()) {
        return Err(AppError::BadRequest("微信返回的 prepay_id 无效".into()));
    }
    Ok(parsed.prepay_id)
}

pub fn mini_program_pay_params(
    creds: &PayCredentials,
    prepay_id: &str,
) -> AppResult<JsapiPayParams> {
    if prepay_id.is_empty() || prepay_id.chars().any(|c| c.is_whitespace()) {
        return Err(AppError::BadRequest("prepay_id 无效".into()));
    }
    let time_stamp = Utc::now().timestamp().to_string();
    let nonce_str = Uuid::new_v4().simple().to_string();
    let package = format!("prepay_id={prepay_id}");
    let message = format!("{}\n{time_stamp}\n{nonce_str}\n{package}\n", creds.app_id);
    let key = parse_private_key(&creds.private_key_pem)
        .map_err(|err| AppError::ServiceUnavailable(format!("商户私钥无法解析：{err}")))?;
    let pay_sign =
        sign_pkcs1(&key, message.as_bytes()).map_err(|err| AppError::ServiceUnavailable(err))?;
    Ok(JsapiPayParams {
        time_stamp,
        nonce_str,
        package,
        sign_type: "RSA".into(),
        pay_sign,
    })
}

pub async fn open_notify(
    state: &AppState,
    headers: NotifyHeaders<'_>,
    body: &[u8],
) -> AppResult<OpenedNotify> {
    let PayMode::Live(creds) = pay_mode(&state.config) else {
        return Err(AppError::Unauthorized("微信支付未启用".into()));
    };
    verify_notify_signature(state, &creds, &headers, body).await?;
    let envelope: NotifyEnvelope = serde_json::from_slice(body)
        .map_err(|_| AppError::BadRequest("回调报文无法解析".into()))?;
    if envelope.event_type != "TRANSACTION.SUCCESS" {
        tracing::info!(event = %envelope.event_type, "ignore non-success wechat notify");
        return Ok(OpenedNotify::Ignored);
    }
    let resource = envelope
        .resource
        .ok_or_else(|| AppError::BadRequest("回调缺少 resource".into()))?;
    if resource.algorithm != "AEAD_AES_256_GCM" {
        return Err(AppError::BadRequest("不支持的加密算法".into()));
    }
    let aad = resource.associated_data.unwrap_or_default();
    let plain = decrypt_aes_gcm(
        &creds.api_v3_key,
        &resource.nonce,
        &aad,
        &resource.ciphertext,
    )
    .map_err(|_| AppError::BadRequest("回调报文解密失败".into()))?;
    let tx: WxTransaction = serde_json::from_slice(&plain)
        .map_err(|_| AppError::BadRequest("交易数据无法解析".into()))?;
    if tx.trade_state != "SUCCESS" {
        return Ok(OpenedNotify::Ignored);
    }
    let verified = verified_from_transaction(tx)?;
    Ok(OpenedNotify::Paid(verified))
}

/// 向微信查单。只有响应签名验证通过且交易成功时才返回交易，供补单入账。
pub async fn query_paid_transaction(
    state: &AppState,
    creds: &PayCredentials,
    out_trade_no: &str,
) -> AppResult<Option<VerifiedTransaction>> {
    if !should_verify_response(&creds.api_base) {
        return Ok(None);
    }
    if !valid_out_trade_no(out_trade_no) {
        return Err(AppError::BadRequest("商户订单号无效".into()));
    }
    let path = format!(
        "/v3/pay/transactions/out-trade-no/{out_trade_no}?mchid={}",
        creds.mch_id
    );
    let resp = signed_request(creds, "GET", &path, "").await?;
    if resp.status.as_u16() == 404 {
        return Ok(None);
    }
    verify_http_signature(state, creds, &resp).await?;
    if !resp.status.is_success() {
        return Err(AppError::BadRequest(format!(
            "查询微信支付订单失败：{}",
            wechat_error_message(&resp.body)
        )));
    }
    let tx: WxTransaction = serde_json::from_slice(&resp.body)
        .map_err(|_| AppError::BadRequest("微信支付查单响应无法解析".into()))?;
    if tx.trade_state != "SUCCESS" {
        return Ok(None);
    }
    Ok(Some(verified_from_transaction(tx)?))
}

fn verified_from_transaction(tx: WxTransaction) -> AppResult<VerifiedTransaction> {
    let transaction_id = tx
        .transaction_id
        .filter(|id| !id.is_empty())
        .ok_or_else(|| AppError::BadRequest("缺少微信支付单号".into()))?;
    let payer_openid = tx
        .payer
        .and_then(|p| p.openid)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| AppError::BadRequest("缺少付款人 openid".into()))?;
    let amount_total = tx
        .amount
        .and_then(|a| a.total)
        .ok_or_else(|| AppError::BadRequest("缺少支付金额".into()))?;
    if tx.out_trade_no.is_empty() || tx.appid.is_empty() || tx.mchid.is_empty() {
        return Err(AppError::BadRequest("交易数据不完整".into()));
    }
    Ok(VerifiedTransaction {
        out_trade_no: tx.out_trade_no,
        transaction_id,
        appid: tx.appid,
        mchid: tx.mchid,
        payer_openid,
        amount_total,
        trade_state: tx.trade_state,
    })
}

async fn verify_notify_signature(
    state: &AppState,
    creds: &PayCredentials,
    headers: &NotifyHeaders<'_>,
    body: &[u8],
) -> AppResult<()> {
    if !timestamp_fresh_at(headers.timestamp, Utc::now().timestamp()) {
        return Err(AppError::Unauthorized("回调时间戳无效".into()));
    }
    if headers.nonce.is_empty() || headers.signature.is_empty() || headers.serial.is_empty() {
        return Err(AppError::Unauthorized("回调签名头缺失".into()));
    }
    let key = public_key_for_serial(state, creds, headers.serial).await?;
    let message = signed_message(headers.timestamp, headers.nonce, body);
    verify_pkcs1(&key, &message, headers.signature)
        .map_err(|_| AppError::Unauthorized("签名校验失败".into()))
}

async fn verify_http_signature(
    state: &AppState,
    creds: &PayCredentials,
    resp: &WxResponse,
) -> AppResult<()> {
    let timestamp = resp
        .header("wechatpay-timestamp")
        .ok_or_else(|| AppError::BadRequest("微信支付响应缺少时间戳".into()))?;
    let nonce = resp
        .header("wechatpay-nonce")
        .ok_or_else(|| AppError::BadRequest("微信支付响应缺少随机串".into()))?;
    let signature = resp
        .header("wechatpay-signature")
        .ok_or_else(|| AppError::BadRequest("微信支付响应缺少签名".into()))?;
    let serial = resp
        .header("wechatpay-serial")
        .ok_or_else(|| AppError::BadRequest("微信支付响应缺少证书序列号".into()))?;
    if !timestamp_fresh_at(&timestamp, Utc::now().timestamp()) {
        return Err(AppError::BadRequest("微信支付响应时间戳无效".into()));
    }
    let key = public_key_for_serial(state, creds, &serial).await?;
    let message = signed_message(&timestamp, &nonce, &resp.body);
    verify_pkcs1(&key, &message, &signature)
        .map_err(|_| AppError::BadRequest("微信支付响应签名校验失败".into()))
}

async fn public_key_for_serial(
    state: &AppState,
    creds: &PayCredentials,
    serial: &str,
) -> AppResult<RsaPublicKey> {
    if !creds.public_key_id.is_empty() && serial == creds.public_key_id {
        return parse_public_key(&creds.public_key_pem)
            .map_err(|err| AppError::ServiceUnavailable(format!("微信支付公钥无法解析：{err}")));
    }
    if let Some(key) = state.pay_certs.get(serial).await {
        return Ok(key);
    }
    refresh_platform_certs(state, creds).await?;
    state.pay_certs.get(serial).await.ok_or_else(|| {
        AppError::Unauthorized("找不到与 Wechatpay-Serial 对应的微信支付平台证书或公钥".into())
    })
}

async fn refresh_platform_certs(state: &AppState, creds: &PayCredentials) -> AppResult<()> {
    if state.pay_certs.recently_fetched().await {
        return Ok(());
    }
    state.pay_certs.mark_fetched().await;
    let resp = signed_request(creds, "GET", "/v3/certificates", "").await?;
    if !resp.status.is_success() {
        return Err(AppError::ServiceUnavailable(format!(
            "下载微信支付平台证书失败：{}",
            wechat_error_message(&resp.body)
        )));
    }
    let list: CertList = serde_json::from_slice(&resp.body)
        .map_err(|_| AppError::ServiceUnavailable("平台证书响应无法解析".into()))?;
    let mut keys = HashMap::new();
    let now = Utc::now();
    for item in list.data {
        if cert_outside_window(&item, now) {
            continue;
        }
        if item.encrypt_certificate.algorithm != "AEAD_AES_256_GCM" {
            continue;
        }
        let aad = item.encrypt_certificate.associated_data.unwrap_or_default();
        let pem = decrypt_aes_gcm(
            &creds.api_v3_key,
            &item.encrypt_certificate.nonce,
            &aad,
            &item.encrypt_certificate.ciphertext,
        )
        .map_err(|_| AppError::ServiceUnavailable("平台证书解密失败".into()))?;
        let pem = String::from_utf8(pem)
            .map_err(|_| AppError::ServiceUnavailable("平台证书不是文本".into()))?;
        let key = parse_public_key(&pem)
            .map_err(|err| AppError::ServiceUnavailable(format!("平台证书公钥无法解析：{err}")))?;
        keys.insert(item.serial_no, key);
    }
    let serial = resp
        .header("wechatpay-serial")
        .ok_or_else(|| AppError::ServiceUnavailable("平台证书响应缺少序列号".into()))?;
    let timestamp = resp
        .header("wechatpay-timestamp")
        .ok_or_else(|| AppError::ServiceUnavailable("平台证书响应缺少时间戳".into()))?;
    let nonce = resp
        .header("wechatpay-nonce")
        .ok_or_else(|| AppError::ServiceUnavailable("平台证书响应缺少随机串".into()))?;
    let signature = resp
        .header("wechatpay-signature")
        .ok_or_else(|| AppError::ServiceUnavailable("平台证书响应缺少签名".into()))?;
    if !timestamp_fresh_at(&timestamp, Utc::now().timestamp()) {
        return Err(AppError::ServiceUnavailable(
            "平台证书响应时间戳无效".into(),
        ));
    }
    let Some(verify_key) = keys.get(&serial).cloned() else {
        return Err(AppError::ServiceUnavailable(
            "平台证书响应的序列号不在证书列表里".into(),
        ));
    };
    let message = signed_message(&timestamp, &nonce, &resp.body);
    verify_pkcs1(&verify_key, &message, &signature)
        .map_err(|_| AppError::ServiceUnavailable("平台证书响应签名校验失败".into()))?;
    state.pay_certs.store(keys).await;
    Ok(())
}

fn cert_outside_window(item: &CertItem, now: DateTime<Utc>) -> bool {
    if let Some(exp) = item
        .expire_time
        .as_deref()
        .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
    {
        if exp < now {
            return true;
        }
    }
    if let Some(start) = item
        .effective_time
        .as_deref()
        .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
    {
        if start > now {
            return true;
        }
    }
    false
}

struct WxResponse {
    status: reqwest::StatusCode,
    headers: reqwest::header::HeaderMap,
    body: Vec<u8>,
}

impl WxResponse {
    fn header(&self, name: &str) -> Option<String> {
        self.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

async fn signed_request(
    creds: &PayCredentials,
    method: &str,
    path_and_query: &str,
    body: &str,
) -> AppResult<WxResponse> {
    let timestamp = Utc::now().timestamp().to_string();
    let nonce = Uuid::new_v4().simple().to_string();
    let message = request_message(method, path_and_query, &timestamp, &nonce, body);
    let key = parse_private_key(&creds.private_key_pem)
        .map_err(|err| AppError::ServiceUnavailable(format!("商户私钥无法解析：{err}")))?;
    let signature = sign_pkcs1(&key, message.as_bytes()).map_err(AppError::ServiceUnavailable)?;
    let authorization = format!(
        "WECHATPAY2-SHA256-RSA2048 mchid=\"{}\",nonce_str=\"{nonce}\",signature=\"{signature}\",timestamp=\"{timestamp}\",serial_no=\"{}\"",
        creds.mch_id, creds.serial_no
    );
    let url = format!("{}{path_and_query}", creds.api_base.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| AppError::Internal(err.into()))?;
    let mut req = client
        .request(method.parse().unwrap_or(reqwest::Method::GET), &url)
        .header("Authorization", authorization)
        .header("Accept", "application/json")
        .header("User-Agent", "pindou-api");
    if !creds.public_key_id.is_empty() {
        req = req.header("Wechatpay-Serial", &creds.public_key_id);
    }
    if method != "GET" {
        req = req
            .header("Content-Type", "application/json")
            .body(body.to_string());
    }
    let resp = req.send().await.map_err(|err| {
        tracing::warn!(error = %err, "wechat pay request failed");
        AppError::ServiceUnavailable("暂时无法连接微信支付，请稍后再试".into())
    })?;
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp
        .bytes()
        .await
        .map_err(|err| AppError::ServiceUnavailable(format!("读取微信支付响应失败：{err}")))?
        .to_vec();
    Ok(WxResponse {
        status,
        headers,
        body,
    })
}

fn resolved_notify_url(config: &Config) -> String {
    let explicit = config.wechat_pay_notify_url.trim();
    if !explicit.is_empty() {
        return explicit.to_string();
    }
    format!(
        "{}/api/v1/pay/wechat/notify",
        config.public_base_url.trim_end_matches('/')
    )
}

fn normalized_api_base(config: &Config) -> String {
    let raw = config.wechat_pay_api_base.trim().trim_end_matches('/');
    if raw.is_empty() {
        OFFICIAL_API.to_string()
    } else {
        raw.to_string()
    }
}

fn official_api(base: &str) -> bool {
    let base = base.trim_end_matches('/');
    base == OFFICIAL_API
}

fn should_verify_response(api_base: &str) -> bool {
    official_api(api_base)
}

fn valid_out_trade_no(value: &str) -> bool {
    (6..=32).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn request_message(
    method: &str,
    path_and_query: &str,
    timestamp: &str,
    nonce: &str,
    body: &str,
) -> String {
    format!("{method}\n{path_and_query}\n{timestamp}\n{nonce}\n{body}\n")
}

fn signed_message(timestamp: &str, nonce: &str, body: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(timestamp.len() + nonce.len() + body.len() + 3);
    msg.extend_from_slice(timestamp.as_bytes());
    msg.push(b'\n');
    msg.extend_from_slice(nonce.as_bytes());
    msg.push(b'\n');
    msg.extend_from_slice(body);
    msg.push(b'\n');
    msg
}

fn timestamp_fresh_at(ts: &str, now: i64) -> bool {
    let Ok(ts) = ts.trim().parse::<i64>() else {
        return false;
    };
    (now - ts).abs() <= TIMESTAMP_WINDOW_SECS
}

fn truncate_bytes(value: &str, max: usize) -> String {
    if value.len() <= max {
        return value.to_string();
    }
    let mut end = max;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

fn wechat_error_message(body: &[u8]) -> String {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return "微信支付返回错误".into();
    };
    let code = value
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("ERROR");
    let message = value
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("微信支付返回错误");
    let text = format!("{code}: {message}");
    text.chars().take(180).collect()
}

fn parse_private_key(pem: &str) -> Result<RsaPrivateKey, String> {
    let pem = pem.trim();
    if let Ok(key) = RsaPrivateKey::from_pkcs8_pem(pem) {
        return Ok(key);
    }
    RsaPrivateKey::from_pkcs1_pem(pem).map_err(|err| err.to_string())
}

fn parse_public_key(pem: &str) -> Result<RsaPublicKey, String> {
    let pem = pem.trim();
    if pem.contains("BEGIN CERTIFICATE") {
        let (_, parsed) =
            x509_parser::pem::parse_x509_pem(pem.as_bytes()).map_err(|err| err.to_string())?;
        let cert = parsed.parse_x509().map_err(|err| err.to_string())?;
        let spki = cert.public_key().raw;
        return RsaPublicKey::from_public_key_der(spki).map_err(|err| err.to_string());
    }
    RsaPublicKey::from_public_key_pem(pem).map_err(|err| err.to_string())
}

fn sign_pkcs1(key: &RsaPrivateKey, message: &[u8]) -> Result<String, String> {
    let signing_key = SigningKey::<Sha256>::new(key.clone());
    let signature = signing_key.sign(message);
    Ok(STANDARD.encode(signature.to_bytes()))
}

fn verify_pkcs1(key: &RsaPublicKey, message: &[u8], signature_b64: &str) -> Result<(), String> {
    let compact: String = signature_b64
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let bytes = STANDARD
        .decode(compact)
        .map_err(|_| "签名不是合法 Base64".to_string())?;
    let signature = Signature::try_from(bytes.as_slice()).map_err(|err| err.to_string())?;
    let verifying_key = VerifyingKey::<Sha256>::new(key.clone());
    verifying_key
        .verify(message, &signature)
        .map_err(|_| "签名不匹配".to_string())
}

fn decrypt_aes_gcm(
    api_v3_key: &str,
    nonce: &str,
    aad: &str,
    ciphertext_b64: &str,
) -> Result<Vec<u8>, String> {
    if api_v3_key.len() != 32 {
        return Err("APIv3 密钥长度必须是 32".into());
    }
    let nonce_bytes = nonce.as_bytes();
    if nonce_bytes.len() != 12 {
        return Err(format!(
            "AES-GCM nonce 长度应为 12 字节，实际 {}",
            nonce_bytes.len()
        ));
    }
    let compact: String = ciphertext_b64
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let ciphertext = STANDARD
        .decode(compact)
        .map_err(|_| "密文不是合法 Base64".to_string())?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(api_v3_key.as_bytes()));
    cipher
        .decrypt(
            Nonce::from_slice(nonce_bytes),
            Payload {
                msg: &ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| "解密失败".to_string())
}

#[doc(hidden)]
pub fn seal_aead_for_tests(
    api_v3_key: &str,
    nonce: &str,
    aad: &str,
    plaintext: &[u8],
) -> Result<String, String> {
    if api_v3_key.len() != 32 {
        return Err("APIv3 密钥长度必须是 32".into());
    }
    if nonce.len() != 12 {
        return Err("nonce".into());
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(api_v3_key.as_bytes()));
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(nonce.as_bytes()),
            Payload {
                msg: plaintext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| "加密失败".to_string())?;
    Ok(STANDARD.encode(ciphertext))
}

#[derive(Deserialize)]
struct PrepayResponse {
    prepay_id: String,
}

#[derive(Deserialize)]
struct NotifyEnvelope {
    event_type: String,
    resource: Option<NotifyResource>,
}

#[derive(Deserialize)]
struct NotifyResource {
    algorithm: String,
    ciphertext: String,
    associated_data: Option<String>,
    nonce: String,
}

#[derive(Deserialize)]
struct WxTransaction {
    appid: String,
    mchid: String,
    out_trade_no: String,
    transaction_id: Option<String>,
    trade_state: String,
    payer: Option<WxPayer>,
    amount: Option<WxAmount>,
}

#[derive(Deserialize)]
struct WxPayer {
    openid: Option<String>,
}

#[derive(Deserialize)]
struct WxAmount {
    total: Option<i64>,
}

#[derive(Deserialize)]
struct CertList {
    data: Vec<CertItem>,
}

#[derive(Deserialize)]
struct CertItem {
    serial_no: String,
    effective_time: Option<String>,
    expire_time: Option<String>,
    encrypt_certificate: EncCert,
}

#[derive(Deserialize)]
struct EncCert {
    algorithm: String,
    nonce: String,
    associated_data: Option<String>,
    ciphertext: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
    use rsa::signature::Signer;

    fn gen_pair() -> (RsaPrivateKey, String, String) {
        let mut rng = rand::thread_rng();
        let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let private_pem = key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
        let public_pem = key
            .to_public_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap();
        (key, private_pem, public_pem)
    }

    #[test]
    fn request_and_pay_messages_match_wechat_layout() {
        assert_eq!(
            request_message(
                "POST",
                "/v3/pay/transactions/jsapi",
                "1554208460",
                "nonce",
                "{\"a\":1}"
            ),
            "POST\n/v3/pay/transactions/jsapi\n1554208460\nnonce\n{\"a\":1}\n"
        );
        assert_eq!(
            request_message("GET", "/v3/certificates", "1554208460", "nonce", ""),
            "GET\n/v3/certificates\n1554208460\nnonce\n\n"
        );
        let message = signed_message("1554208460", "nonce", b"{}");
        assert_eq!(message, b"1554208460\nnonce\n{}\n");
    }

    #[test]
    fn sign_and_verify_roundtrip_rejects_tamper() {
        let (key, _, public_pem) = gen_pair();
        let public = parse_public_key(&public_pem).unwrap();
        let message = b"wxapp\n1600000000\nnonce\nprepay_id=abc\n";
        let signature = sign_pkcs1(&key, message).unwrap();
        assert!(verify_pkcs1(&public, message, &signature).is_ok());
        let mut tampered = message.to_vec();
        tampered[0] ^= 1;
        assert!(verify_pkcs1(&public, &tampered, &signature).is_err());
        assert!(verify_pkcs1(&public, message, "aaaa").is_err());
    }

    #[test]
    fn aead_roundtrip_and_bad_tag() {
        let key = "0123456789abcdef0123456789abcdef";
        let nonce = "0123456789ab";
        let aad = "transaction";
        let plain = br#"{"trade_state":"SUCCESS","amount":{"total":600}}"#;
        let sealed = seal_aead_for_tests(key, nonce, aad, plain).unwrap();
        let opened = decrypt_aes_gcm(key, nonce, aad, &sealed).unwrap();
        assert_eq!(opened, plain);
        assert!(decrypt_aes_gcm(key, nonce, "other", &sealed).is_err());
        let mut broken = sealed.clone();
        let idx = broken.len() - 2;
        broken.replace_range(idx..idx + 1, if broken.ends_with('A') { "B" } else { "A" });
        assert!(decrypt_aes_gcm(key, nonce, aad, &broken).is_err());
        assert!(decrypt_aes_gcm(key, "short", aad, &sealed).is_err());
    }

    #[test]
    fn timestamp_window() {
        assert!(timestamp_fresh_at("1000", 1000));
        assert!(timestamp_fresh_at("1000", 1299));
        assert!(!timestamp_fresh_at("1000", 1301));
        assert!(!timestamp_fresh_at("nope", 1000));
    }

    #[test]
    fn pay_mode_mock_partial_and_live() {
        let mut config = Config::from_env();
        config.force_mock_pay();
        assert!(matches!(pay_mode(&config), PayMode::Mock));

        config.pay_credentials_present = true;
        config.wechat_mch_id = "1900000109".into();
        assert!(matches!(pay_mode(&config), PayMode::Misconfigured(_)));

        let (_, private_pem, public_pem) = gen_pair();
        config.wechat_app_id = "wx_test_appid".into();
        config.wechat_mch_serial_no = "SERIAL123".into();
        config.wechat_mch_private_key = private_pem;
        config.wechat_pay_api_v3_key = "0123456789abcdef0123456789abcdef".into();
        config.wechat_pay_public_key = public_pem;
        config.wechat_pay_public_key_id = "PUB_KEY_ID_TEST".into();
        config.wechat_pay_notify_url = "https://pay.example.com/api/v1/pay/wechat/notify".into();
        config.wechat_pay_api_base = OFFICIAL_API.into();
        assert!(matches!(pay_mode(&config), PayMode::Live(_)));

        config.wechat_pay_notify_url = "http://127.0.0.1/api/v1/pay/wechat/notify".into();
        assert!(matches!(pay_mode(&config), PayMode::Misconfigured(_)));
    }

    #[test]
    fn certificate_pem_parses_to_the_same_public_key() {
        let openssl = std::process::Command::new("openssl")
            .arg("version")
            .output();
        if openssl.is_err() {
            return;
        }
        let (_, private_pem, public_pem) = gen_pair();
        let dir = std::env::temp_dir().join(format!("pindou-pay-cert-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let key_path = dir.join("key.pem");
        let cert_path = dir.join("cert.pem");
        std::fs::write(&key_path, &private_pem).unwrap();
        let output = std::process::Command::new("openssl")
            .arg("req")
            .arg("-new")
            .arg("-x509")
            .arg("-key")
            .arg(&key_path)
            .arg("-out")
            .arg(&cert_path)
            .arg("-days")
            .arg("1")
            .arg("-subj")
            .arg("/CN=pindou-test")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let cert = std::fs::read_to_string(&cert_path).unwrap();
        let from_cert = parse_public_key(&cert).expect("cert");
        let from_spki = parse_public_key(&public_pem).expect("spki");
        assert_eq!(from_cert, from_spki);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn signer_trait_is_linked() {
        let (key, _, _) = gen_pair();
        let signing_key = SigningKey::<Sha256>::new(key);
        let _ = signing_key.sign(b"ok");
    }
}
