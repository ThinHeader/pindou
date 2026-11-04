//! 小程序虚拟支付（米大师道具直购）。
//!
//! `wx.requestVirtualPayment` 的 `signData` 必须原样参与 HMAC。
//! `paySig` 用对应环境的 AppKey，`signature` 用用户的 `session_key`。
//! AppKey 和 session_key 只留在服务端。

use std::sync::Arc;
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use serde::Serialize;
use serde_json::Value;
use sha2::Sha256;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

const OFFICIAL_API: &str = "https://api.weixin.qq.com";
pub const REQUEST_VIRTUAL_PAYMENT: &str = "requestVirtualPayment";
const QUERY_URI: &str = "/xpay/query_order";
const NOTIFY_URI: &str = "/xpay/notify_provide_goods";
const REFUND_URI: &str = "/xpay/refund_order";
/// 支付签名校验失败。这两种不能当成「订单不存在」。
const PAY_SIG_ERR_A: i64 = 268490001;
const PAY_SIG_ERR_B: i64 = 268490003;

#[derive(Clone, Default)]
pub struct AccessTokenStore {
    inner: Arc<RwLock<Option<CachedToken>>>,
}

struct CachedToken {
    api_base: String,
    app_id: String,
    token: String,
    expires_at: Instant,
}

pub struct VirtualCreds {
    pub offer_id: String,
    pub app_key: String,
    pub env: u8,
    pub app_id: String,
    pub app_secret: String,
    pub api_base: String,
    pub msg_token: String,
    product_small: String,
    product_popular: String,
    product_large: String,
}

impl VirtualCreds {
    pub fn product_id(&self, pack_code: &str) -> Result<String, String> {
        let raw = match pack_code {
            "small" if !self.product_small.is_empty() => self.product_small.as_str(),
            "popular" if !self.product_popular.is_empty() => self.product_popular.as_str(),
            "large" if !self.product_large.is_empty() => self.product_large.as_str(),
            other => other,
        };
        if valid_product_id(raw) {
            Ok(raw.to_string())
        } else {
            Err("道具 ID 无效".into())
        }
    }
}

pub enum VirtualMode {
    Unset,
    Misconfigured(String),
    Live(VirtualCreds),
}

#[derive(Debug, Serialize)]
pub struct VirtualPayParams {
    #[serde(rename = "signData")]
    pub sign_data: String,
    #[serde(rename = "paySig")]
    pub pay_sig: String,
    pub signature: String,
    pub mode: String,
}

#[derive(Debug)]
pub struct PaidFacts {
    pub transaction_id: String,
    pub wx_order_id: Option<String>,
    pub order_fee: Option<i64>,
    pub paid_fee: Option<i64>,
}

pub enum QueryOutcome {
    Paid(PaidFacts),
    NotReady,
    ClosedOrRefunded,
}

#[derive(Debug)]
pub struct DeliverPush {
    pub out_trade_no: String,
    pub openid: String,
    pub env: u8,
    pub product_id: String,
    pub quantity: i64,
    pub orig_price: i64,
    pub actual_price: i64,
    pub attach: String,
    pub transaction_id: String,
}

#[derive(Debug)]
pub struct RefundPush {
    pub out_trade_no: String,
    pub refund_order_id: String,
    pub openid: String,
    pub refund_fee: i64,
    pub success: bool,
}

pub enum ParsedPush {
    Deliver(DeliverPush),
    Refund(RefundPush),
    Ignored { event: String },
}

#[derive(Serialize)]
struct GoodsSignData<'a> {
    #[serde(rename = "offerId")]
    offer_id: &'a str,
    #[serde(rename = "buyQuantity")]
    buy_quantity: i32,
    env: u8,
    #[serde(rename = "currencyType")]
    currency_type: &'static str,
    #[serde(rename = "productId")]
    product_id: &'a str,
    #[serde(rename = "goodsPrice")]
    goods_price: i32,
    #[serde(rename = "outTradeNo")]
    out_trade_no: &'a str,
    attach: &'a str,
}

#[derive(Serialize)]
struct QueryBody<'a> {
    openid: &'a str,
    env: u8,
    order_id: &'a str,
}

#[derive(Serialize)]
struct ProvideBody<'a> {
    order_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    wx_order_id: Option<&'a str>,
    env: u8,
}

/// 字段顺序参与 pay_sig，不要重排。
#[derive(Serialize)]
struct RefundBody<'a> {
    openid: &'a str,
    order_id: &'a str,
    refund_order_id: &'a str,
    left_fee: i64,
    refund_fee: i64,
    biz_meta: &'a str,
    refund_reason: &'a str,
    req_from: &'a str,
    env: u8,
}

#[derive(Debug)]
pub struct CashOrder {
    pub status: i64,
    pub left_fee: Option<i64>,
    pub order_fee: Option<i64>,
    pub paid_fee: Option<i64>,
    pub env_matches: bool,
}

pub enum CashLookup {
    Found(CashOrder),
    NotFound,
}

pub struct RefundLaunch {
    pub errcode: i64,
    pub refund_wx_order_id: String,
}

pub fn virtual_mode(config: &Config) -> VirtualMode {
    if !virtual_present(config) {
        return VirtualMode::Unset;
    }
    match resolve(config) {
        Ok(creds) => VirtualMode::Live(creds),
        Err(reason) => VirtualMode::Misconfigured(reason),
    }
}

pub fn notify_url(config: &Config) -> String {
    format!(
        "{}/api/v1/pay/virtual/notify",
        config.public_base_url.trim_end_matches('/')
    )
}

pub fn attach_for(pack_code: &str, out_trade_no: &str) -> String {
    format!("pack={pack_code}&order={out_trade_no}")
}

pub fn sign_data_json(
    creds: &VirtualCreds,
    product_id: &str,
    goods_price: i32,
    out_trade_no: &str,
    attach: &str,
) -> AppResult<String> {
    if !valid_out_trade_no(out_trade_no) {
        return Err(AppError::BadRequest("商户订单号无效".into()));
    }
    if goods_price <= 0 {
        return Err(AppError::BadRequest("道具价格无效".into()));
    }
    let payload = GoodsSignData {
        offer_id: &creds.offer_id,
        buy_quantity: 1,
        env: creds.env,
        currency_type: "CNY",
        product_id,
        goods_price,
        out_trade_no,
        attach,
    };
    serde_json::to_string(&payload).map_err(|err| AppError::Internal(err.into()))
}

pub fn pay_sig(app_key: &str, uri: &str, body: &str) -> Result<String, String> {
    hmac_sha256_hex(app_key, &format!("{uri}&{body}"))
}

pub fn user_signature(session_key: &str, body: &str) -> Result<String, String> {
    hmac_sha256_hex(session_key, body)
}

pub fn message_signature(token: &str, timestamp: &str, nonce: &str) -> String {
    let mut parts = [token, timestamp, nonce];
    parts.sort_unstable();
    let mut hasher = <sha1::Sha1 as sha1::Digest>::new();
    sha1::Digest::update(&mut hasher, parts[0].as_bytes());
    sha1::Digest::update(&mut hasher, parts[1].as_bytes());
    sha1::Digest::update(&mut hasher, parts[2].as_bytes());
    hex::encode(sha1::Digest::finalize(hasher))
}

pub fn message_signature_matches(token: &str, timestamp: &str, nonce: &str, given: &str) -> bool {
    if token.is_empty() || timestamp.is_empty() || nonce.is_empty() || given.is_empty() {
        return false;
    }
    hex_eq(&message_signature(token, timestamp, nonce), given.trim())
}

pub fn valid_out_trade_no(value: &str) -> bool {
    (8..=32).contains(&value.len())
        && !value.starts_with('_')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '|' | '*' | '@'))
}

pub fn parse_push(body: &[u8]) -> Result<ParsedPush, String> {
    let text = std::str::from_utf8(body)
        .map_err(|_| "推送不是 UTF-8".to_string())?
        .trim_start_matches('\u{feff}')
        .trim();
    if text.is_empty() {
        return Err("推送为空".into());
    }
    if text.starts_with('<') {
        return parse_xml_push(text);
    }
    if text.starts_with('{') {
        let value: Value =
            serde_json::from_str(text).map_err(|_| "推送 JSON 无法解析".to_string())?;
        return parse_json_push(&value);
    }
    Err("推送格式无法识别".into())
}

pub async fn query_order(
    state: &AppState,
    creds: &VirtualCreds,
    openid: &str,
    out_trade_no: &str,
) -> AppResult<QueryOutcome> {
    if openid.is_empty() || !valid_out_trade_no(out_trade_no) {
        return Err(AppError::BadRequest("查单参数无效".into()));
    }
    let body = serde_json::to_string(&QueryBody {
        openid,
        env: creds.env,
        order_id: out_trade_no,
    })
    .map_err(|err| AppError::Internal(err.into()))?;
    let (status, bytes) = post_xpay(state, creds, QUERY_URI, body).await?;
    if !status.is_success() {
        tracing::warn!(http = status.as_u16(), "virtual query http failed");
        return Err(AppError::ServiceUnavailable("查询虚拟支付订单失败".into()));
    }
    interpret_query(&bytes, creds.env, out_trade_no)
}

pub async fn lookup_cash_order(
    state: &AppState,
    creds: &VirtualCreds,
    openid: &str,
    order_id: &str,
) -> AppResult<CashLookup> {
    if openid.is_empty() || order_id.is_empty() {
        return Err(AppError::BadRequest("查单参数无效".into()));
    }
    let body = serde_json::to_string(&QueryBody {
        openid,
        env: creds.env,
        order_id,
    })
    .map_err(|err| AppError::Internal(err.into()))?;
    let (status, bytes) = post_xpay(state, creds, QUERY_URI, body).await?;
    if !status.is_success() {
        tracing::warn!(http = status.as_u16(), "virtual lookup http failed");
        return Err(AppError::ServiceUnavailable("查询虚拟支付订单失败".into()));
    }
    interpret_lookup(&bytes, creds.env)
}

/// 启动整单退款任务。`errcode == 0` 只表示微信受理，不表示钱已退回。
pub async fn launch_refund(
    state: &AppState,
    creds: &VirtualCreds,
    openid: &str,
    out_trade_no: &str,
    refund_order_id: &str,
    left_fee: i64,
    biz_meta: &str,
) -> AppResult<RefundLaunch> {
    if openid.is_empty()
        || !valid_out_trade_no(out_trade_no)
        || !valid_refund_order_id(refund_order_id)
    {
        return Err(AppError::BadRequest("退款参数无效".into()));
    }
    if left_fee <= 0 {
        return Err(AppError::BadRequest("没有可退金额".into()));
    }
    let body = refund_body_json(
        openid,
        out_trade_no,
        refund_order_id,
        left_fee,
        left_fee,
        biz_meta,
        creds.env,
    )?;
    let (status, bytes) = post_xpay(state, creds, REFUND_URI, body).await?;
    if !status.is_success() {
        tracing::warn!(http = status.as_u16(), "virtual refund http failed");
        return Err(AppError::ServiceUnavailable("微信退款请求失败".into()));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| AppError::ServiceUnavailable("微信退款响应无法解析".into()))?;
    let errcode = value.get("errcode").and_then(json_i64).unwrap_or(0);
    if errcode == PAY_SIG_ERR_A || errcode == PAY_SIG_ERR_B {
        tracing::warn!(errcode, "virtual refund signature rejected");
        return Err(AppError::ServiceUnavailable("微信退款签名被拒绝".into()));
    }
    if errcode != 0 {
        tracing::warn!(errcode, "virtual refund not accepted");
    }
    Ok(RefundLaunch {
        errcode,
        refund_wx_order_id: json_string(&value, &["refund_wx_order_id", "refundWxOrderId"]),
    })
}

pub fn valid_refund_order_id(value: &str) -> bool {
    (8..=32).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn refund_body_json(
    openid: &str,
    out_trade_no: &str,
    refund_order_id: &str,
    left_fee: i64,
    refund_fee: i64,
    biz_meta: &str,
    env: u8,
) -> AppResult<String> {
    serde_json::to_string(&RefundBody {
        openid,
        order_id: out_trade_no,
        refund_order_id,
        left_fee,
        refund_fee,
        biz_meta,
        refund_reason: "2",
        req_from: "1",
        env,
    })
    .map_err(|err| AppError::Internal(err.into()))
}

async fn post_xpay(
    state: &AppState,
    creds: &VirtualCreds,
    uri: &str,
    body: String,
) -> AppResult<(reqwest::StatusCode, Vec<u8>)> {
    let pay_sig = pay_sig(creds.app_key.as_str(), uri, &body)
        .map_err(|_| AppError::ServiceUnavailable("虚拟支付签名失败".into()))?;
    let token = access_token(state, creds).await?;
    let url = format!("{}{uri}", creds.api_base.trim_end_matches('/'));
    let resp = http_client()?
        .post(url)
        .query(&[
            ("access_token", token.as_str()),
            ("pay_sig", pay_sig.as_str()),
        ])
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(transport_unavailable)?;
    let status = resp.status();
    let bytes = read_limited(resp).await?;
    Ok((status, bytes))
}

pub async fn notify_delivered(
    state: &AppState,
    creds: &VirtualCreds,
    out_trade_no: &str,
    wx_order_id: Option<&str>,
) -> AppResult<()> {
    let wx_order_id = wx_order_id.filter(|id| !id.is_empty());
    let body = serde_json::to_string(&ProvideBody {
        order_id: out_trade_no,
        wx_order_id,
        env: creds.env,
    })
    .map_err(|err| AppError::Internal(err.into()))?;
    let (status, bytes) = post_xpay(state, creds, NOTIFY_URI, body).await?;
    if !status.is_success() {
        return Err(AppError::ServiceUnavailable("通知微信发货失败".into()));
    }
    if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
        if let Some(code) = value.get("errcode").and_then(|v| v.as_i64()) {
            if code != 0 {
                tracing::warn!(errcode = code, "notify_provide_goods rejected");
                return Err(AppError::ServiceUnavailable("通知微信发货失败".into()));
            }
        }
    }
    Ok(())
}

fn interpret_query(bytes: &[u8], env: u8, out_trade_no: &str) -> AppResult<QueryOutcome> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| AppError::BadRequest("虚拟支付查单响应无法解析".into()))?;
    let errcode = value.get("errcode").and_then(|v| v.as_i64()).unwrap_or(0);
    if errcode == PAY_SIG_ERR_A || errcode == PAY_SIG_ERR_B {
        tracing::warn!(errcode, "virtual query rejected");
        return Err(AppError::ServiceUnavailable("查询虚拟支付订单失败".into()));
    }
    if errcode != 0 {
        tracing::info!(errcode, "virtual query not ready");
        return Ok(QueryOutcome::NotReady);
    }
    let Some(order) = value.get("order").filter(|v| v.is_object()) else {
        return Ok(QueryOutcome::NotReady);
    };
    if !env_type_matches(order.get("env_type").and_then(json_i64), env) {
        tracing::warn!("virtual query env_type mismatch");
        return Ok(QueryOutcome::NotReady);
    }
    let status = order.get("status").and_then(json_i64).unwrap_or(0);
    if matches!(status, 5 | 6 | 8) {
        return Ok(QueryOutcome::ClosedOrRefunded);
    }
    if !matches!(status, 2 | 3 | 4 | 7 | 9 | 10) {
        return Ok(QueryOutcome::NotReady);
    }
    let wx_order_id = json_string(order, &["wx_order_id", "wxOrderId"]);
    let transaction_id = first_non_empty([
        json_string(order, &["wxpay_order_id", "wxpayOrderId"]),
        json_string(order, &["channel_order_id", "channelOrderId"]),
        wx_order_id.clone(),
        format!("xpay:{out_trade_no}"),
    ]);
    let wx_order_id = if wx_order_id.is_empty() {
        None
    } else {
        Some(wx_order_id)
    };
    Ok(QueryOutcome::Paid(PaidFacts {
        transaction_id,
        wx_order_id,
        order_fee: order.get("order_fee").and_then(json_i64),
        paid_fee: order.get("paid_fee").and_then(json_i64),
    }))
}

fn interpret_lookup(bytes: &[u8], env: u8) -> AppResult<CashLookup> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| AppError::BadRequest("虚拟支付查单响应无法解析".into()))?;
    let errcode = value.get("errcode").and_then(json_i64).unwrap_or(0);
    if errcode == PAY_SIG_ERR_A || errcode == PAY_SIG_ERR_B {
        tracing::warn!(errcode, "virtual lookup rejected");
        return Err(AppError::ServiceUnavailable("查询虚拟支付订单失败".into()));
    }
    if errcode != 0 {
        return Ok(CashLookup::NotFound);
    }
    let Some(order) = value.get("order").filter(|v| v.is_object()) else {
        return Ok(CashLookup::NotFound);
    };
    Ok(CashLookup::Found(CashOrder {
        status: order.get("status").and_then(json_i64).unwrap_or(0),
        left_fee: order.get("left_fee").and_then(json_i64),
        order_fee: order.get("order_fee").and_then(json_i64),
        paid_fee: order.get("paid_fee").and_then(json_i64),
        env_matches: env_type_matches(order.get("env_type").and_then(json_i64), env),
    }))
}

fn env_type_matches(env_type: Option<i64>, env: u8) -> bool {
    match env_type {
        None | Some(0) => true,
        Some(1) => env == 0,
        Some(2) => env == 1,
        Some(_) => false,
    }
}

async fn access_token(state: &AppState, creds: &VirtualCreds) -> AppResult<String> {
    if let Some(token) = state.wx_tokens.fresh(&creds.api_base, &creds.app_id).await {
        return Ok(token);
    }
    let url = format!("{}/cgi-bin/token", creds.api_base.trim_end_matches('/'));
    let resp = http_client()?
        .get(url)
        .query(&[
            ("grant_type", "client_credential"),
            ("appid", creds.app_id.as_str()),
            ("secret", creds.app_secret.as_str()),
        ])
        .send()
        .await
        .map_err(transport_unavailable)?;
    let status = resp.status();
    let bytes = read_limited(resp).await?;
    if !status.is_success() {
        return Err(AppError::ServiceUnavailable(
            "获取微信 access_token 失败".into(),
        ));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| AppError::ServiceUnavailable("微信 access_token 响应无法解析".into()))?;
    if value.get("errcode").and_then(|v| v.as_i64()).unwrap_or(0) != 0 {
        let code = value.get("errcode").and_then(|v| v.as_i64()).unwrap_or(-1);
        tracing::warn!(errcode = code, "virtual access_token rejected");
        return Err(AppError::ServiceUnavailable(
            "获取微信 access_token 失败".into(),
        ));
    }
    let token = value
        .get("access_token")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::ServiceUnavailable("获取微信 access_token 失败".into()))?
        .to_string();
    let expires_in = value
        .get("expires_in")
        .and_then(|v| v.as_u64())
        .unwrap_or(7200);
    let ttl = Duration::from_secs(expires_in.saturating_sub(180).max(30));
    state
        .wx_tokens
        .store(&creds.api_base, &creds.app_id, token.clone(), ttl)
        .await;
    Ok(token)
}

impl AccessTokenStore {
    async fn fresh(&self, api_base: &str, app_id: &str) -> Option<String> {
        let guard = self.inner.read().await;
        let cached = guard.as_ref()?;
        if cached.api_base == api_base
            && cached.app_id == app_id
            && cached.expires_at > Instant::now()
        {
            Some(cached.token.clone())
        } else {
            None
        }
    }

    async fn store(&self, api_base: &str, app_id: &str, token: String, ttl: Duration) {
        *self.inner.write().await = Some(CachedToken {
            api_base: api_base.to_string(),
            app_id: app_id.to_string(),
            token,
            expires_at: Instant::now() + ttl,
        });
    }
}

fn http_client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| AppError::Internal(err.into()))
}

fn transport_unavailable(err: reqwest::Error) -> AppError {
    tracing::warn!(
        timeout = err.is_timeout(),
        connect = err.is_connect(),
        status = err.status().map(|code| code.as_u16()),
        "virtual pay http failed"
    );
    AppError::ServiceUnavailable("暂时无法连接微信，请稍后再试".into())
}

async fn read_limited(resp: reqwest::Response) -> AppResult<Vec<u8>> {
    let bytes = resp.bytes().await.map_err(transport_unavailable)?;
    if bytes.len() > 64 * 1024 {
        return Err(AppError::BadRequest("微信响应过大".into()));
    }
    Ok(bytes.to_vec())
}

fn virtual_present(config: &Config) -> bool {
    !config.wx_virtual_offer_id.trim().is_empty()
        || !config.wx_virtual_appkey_sandbox.trim().is_empty()
        || !config.wx_virtual_appkey.trim().is_empty()
        || !config.wx_virtual_appkey_prod.trim().is_empty()
        || !config.wx_virtual_env.trim().is_empty()
        || !config.wx_virtual_product_small.trim().is_empty()
        || !config.wx_virtual_product_popular.trim().is_empty()
        || !config.wx_virtual_product_large.trim().is_empty()
        || !config.wx_virtual_msg_token.trim().is_empty()
}

fn resolve(config: &Config) -> Result<VirtualCreds, String> {
    let offer_id = config.wx_virtual_offer_id.trim();
    if offer_id.is_empty() {
        return Err("缺少 WX_VIRTUAL_OFFER_ID".into());
    }
    if config.wx_appid.trim().is_empty() || config.wx_secret.trim().is_empty() {
        return Err("缺少 WX_APPID 或 WX_SECRET".into());
    }
    if !config.wechat_app_id.trim().is_empty()
        && config.wechat_app_id.trim() != config.wx_appid.trim()
    {
        return Err("WECHAT_APP_ID 与 WX_APPID 不一致".into());
    }
    let env = match config.wx_virtual_env.trim() {
        "" | "1" => 1,
        "0" => 0,
        _ => return Err("WX_VIRTUAL_ENV 只能是 0 或 1".into()),
    };
    let app_key = if env == 1 {
        let key = config.wx_virtual_appkey_sandbox.trim();
        if key.is_empty() {
            return Err("沙箱环境缺少 WX_VIRTUAL_APPKEY_SANDBOX".into());
        }
        key.to_string()
    } else {
        prod_key(config)?
    };
    let api_base = {
        let raw = config.wx_virtual_api_base.trim().trim_end_matches('/');
        if raw.is_empty() {
            OFFICIAL_API.to_string()
        } else {
            raw.to_string()
        }
    };
    Ok(VirtualCreds {
        offer_id: offer_id.to_string(),
        app_key,
        env,
        app_id: config.wx_appid.trim().to_string(),
        app_secret: config.wx_secret.trim().to_string(),
        api_base,
        msg_token: config.wx_virtual_msg_token.trim().to_string(),
        product_small: config.wx_virtual_product_small.trim().to_string(),
        product_popular: config.wx_virtual_product_popular.trim().to_string(),
        product_large: config.wx_virtual_product_large.trim().to_string(),
    })
}

fn prod_key(config: &Config) -> Result<String, String> {
    let primary = config.wx_virtual_appkey.trim();
    let alias = config.wx_virtual_appkey_prod.trim();
    if !primary.is_empty() && !alias.is_empty() && primary != alias {
        return Err("WX_VIRTUAL_APPKEY 与 WX_VIRTUAL_APPKEY_PROD 不一致".into());
    }
    if !primary.is_empty() {
        return Ok(primary.to_string());
    }
    if !alias.is_empty() {
        return Ok(alias.to_string());
    }
    Err("现网环境缺少 WX_VIRTUAL_APPKEY".into())
}

fn valid_product_id(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn hmac_sha256_hex(key: &str, message: &str) -> Result<String, String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).map_err(|_| "hmac".to_string())?;
    mac.update(message.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn hex_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a.to_ascii_lowercase() ^ b.to_ascii_lowercase();
    }
    diff == 0
}

fn parse_json_push(value: &Value) -> Result<ParsedPush, String> {
    let event = json_string(value, &["Event", "event"]);
    if event.is_empty() {
        return Err("推送缺少 Event".into());
    }
    if event == "xpay_refund_notify" {
        return parse_refund_json(value);
    }
    if event != "xpay_goods_deliver_notify" {
        return Ok(ParsedPush::Ignored { event });
    }
    let goods = value
        .get("GoodsInfo")
        .or_else(|| value.get("goodsInfo"))
        .unwrap_or(value);
    let pay = value
        .get("WeChatPayInfo")
        .or_else(|| value.get("weChatPayInfo"))
        .unwrap_or(&Value::Null);
    let out_trade_no = json_string(value, &["OutTradeNo", "outTradeNo"]);
    let openid = json_string(value, &["OpenId", "openId", "openid"]);
    let env = json_i64_at(value, &["Env", "env"]).ok_or_else(|| "推送缺少 Env".to_string())?;
    let product_id = json_string(goods, &["ProductId", "productId"]);
    let quantity = json_i64_at(goods, &["Quantity", "quantity"]).unwrap_or(0);
    let orig_price = json_i64_at(goods, &["OrigPrice", "origPrice"]).unwrap_or(0);
    let actual_price = json_i64_at(goods, &["ActualPrice", "actualPrice"]).unwrap_or(0);
    let attach = json_string(goods, &["Attach", "attach"]);
    let transaction_id = json_string(pay, &["TransactionId", "transactionId"]);
    if out_trade_no.is_empty() || openid.is_empty() || product_id.is_empty() {
        return Err("发货推送缺少订单、用户或道具".into());
    }
    if !(0..=1).contains(&env) {
        return Err("发货推送 Env 无效".into());
    }
    let transaction_id = if transaction_id.is_empty() {
        let mch = json_string(pay, &["MchOrderNo", "mchOrderNo"]);
        if mch.is_empty() {
            format!("xpay:{out_trade_no}")
        } else {
            mch
        }
    } else {
        transaction_id
    };
    Ok(ParsedPush::Deliver(DeliverPush {
        out_trade_no,
        openid,
        env: env as u8,
        product_id,
        quantity,
        orig_price,
        actual_price,
        attach,
        transaction_id,
    }))
}

fn parse_xml_push(text: &str) -> Result<ParsedPush, String> {
    let event = xml_tag(text, "Event").unwrap_or_default();
    if event.is_empty() {
        return Err("推送缺少 Event".into());
    }
    if event == "xpay_refund_notify" {
        return parse_refund_xml(text);
    }
    if event != "xpay_goods_deliver_notify" {
        return Ok(ParsedPush::Ignored { event });
    }
    let out_trade_no = xml_tag(text, "OutTradeNo").unwrap_or_default();
    let openid = xml_tag(text, "OpenId").unwrap_or_default();
    let env: i64 = xml_tag(text, "Env")
        .unwrap_or_default()
        .parse()
        .map_err(|_| "推送 Env 无效".to_string())?;
    let product_id = xml_tag(text, "ProductId").unwrap_or_default();
    let quantity = xml_tag(text, "Quantity")
        .unwrap_or_default()
        .parse()
        .unwrap_or(0);
    let orig_price = xml_tag(text, "OrigPrice")
        .unwrap_or_default()
        .parse()
        .unwrap_or(0);
    let actual_price = xml_tag(text, "ActualPrice")
        .unwrap_or_default()
        .parse()
        .unwrap_or(0);
    let attach = xml_tag(text, "Attach").unwrap_or_default();
    let transaction_id = xml_tag(text, "TransactionId").unwrap_or_default();
    let mch_order_no = xml_tag(text, "MchOrderNo").unwrap_or_default();
    if out_trade_no.is_empty() || openid.is_empty() || product_id.is_empty() {
        return Err("发货推送缺少订单、用户或道具".into());
    }
    if !(0..=1).contains(&env) {
        return Err("发货推送 Env 无效".into());
    }
    let transaction_id = if !transaction_id.is_empty() {
        transaction_id
    } else if !mch_order_no.is_empty() {
        mch_order_no
    } else {
        format!("xpay:{out_trade_no}")
    };
    Ok(ParsedPush::Deliver(DeliverPush {
        out_trade_no,
        openid,
        env: env as u8,
        product_id,
        quantity,
        orig_price,
        actual_price,
        attach,
        transaction_id,
    }))
}

fn parse_refund_json(value: &Value) -> Result<ParsedPush, String> {
    let out_trade_no = json_string(
        value,
        &["MchOrderId", "mchOrderId", "OutTradeNo", "outTradeNo"],
    );
    let refund_order_id = json_string(value, &["MchRefundId", "mchRefundId"]);
    let openid = json_string(value, &["OpenId", "openId", "openid"]);
    let Some(ret) = json_i64_at(value, &["RetCode", "retCode"]) else {
        return Ok(ParsedPush::Ignored {
            event: "xpay_refund_notify".into(),
        });
    };
    if out_trade_no.is_empty() {
        return Ok(ParsedPush::Ignored {
            event: "xpay_refund_notify".into(),
        });
    }
    Ok(ParsedPush::Refund(RefundPush {
        out_trade_no,
        refund_order_id,
        openid,
        refund_fee: json_i64_at(value, &["RefundFee", "refundFee"]).unwrap_or(0),
        success: ret == 0,
    }))
}

fn parse_refund_xml(text: &str) -> Result<ParsedPush, String> {
    let out_trade_no = xml_tag(text, "MchOrderId")
        .filter(|value| !value.is_empty())
        .or_else(|| xml_tag(text, "OutTradeNo"))
        .unwrap_or_default();
    let ret = xml_tag(text, "RetCode").unwrap_or_default();
    if out_trade_no.is_empty() || ret.is_empty() {
        return Ok(ParsedPush::Ignored {
            event: "xpay_refund_notify".into(),
        });
    }
    let ret_code: i64 = ret
        .parse()
        .map_err(|_| "退款推送 RetCode 无效".to_string())?;
    Ok(ParsedPush::Refund(RefundPush {
        out_trade_no,
        refund_order_id: xml_tag(text, "MchRefundId").unwrap_or_default(),
        openid: xml_tag(text, "OpenId").unwrap_or_default(),
        refund_fee: xml_tag(text, "RefundFee")
            .unwrap_or_default()
            .parse()
            .unwrap_or(0),
        success: ret_code == 0,
    }))
}

fn xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let rest = &xml[start..];
    let end = rest.find(&close)?;
    Some(decode_xml_text(rest[..end].trim()))
}

fn decode_xml_text(raw: &str) -> String {
    let inner = raw
        .strip_prefix("<![CDATA[")
        .and_then(|value| value.strip_suffix("]]>"))
        .unwrap_or(raw)
        .trim();
    inner
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn json_string(value: &Value, names: &[&str]) -> String {
    for name in names {
        if let Some(item) = value.get(*name) {
            if let Some(text) = item.as_str() {
                return text.trim().to_string();
            }
            if let Some(number) = item.as_i64() {
                return number.to_string();
            }
        }
    }
    String::new()
}

fn json_i64_at(value: &Value, names: &[&str]) -> Option<i64> {
    for name in names {
        if let Some(item) = value.get(*name) {
            if let Some(number) = json_i64(item) {
                return Some(number);
            }
        }
    }
    None
}

fn json_i64(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    if let Some(number) = value.as_u64() {
        return i64::try_from(number).ok();
    }
    value.as_str().and_then(|text| text.trim().parse().ok())
}

fn first_non_empty(items: [String; 4]) -> String {
    items
        .into_iter()
        .find(|item| !item.is_empty())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_config() -> Config {
        let mut config = Config::from_env();
        config.force_mock_pay();
        config.wx_appid.clear();
        config.wx_secret.clear();
        config.wechat_app_id.clear();
        config
    }

    fn live_sandbox() -> Config {
        let mut config = blank_config();
        config.wx_appid = "wx_test_app".into();
        config.wx_secret = "wx_test_secret".into();
        config.wx_virtual_offer_id = "offer-test".into();
        config.wx_virtual_appkey_sandbox = "sandbox-key".into();
        config
    }

    #[test]
    fn official_hmac_vectors_and_request_virtual_payment() {
        let post_body = "{\"openid\": \"xxx\", \"user_ip\": \"127.0.0.1\", \"env\": 0}";
        let pay = pay_sig("12345", "/xpay/query_user_balance", post_body).unwrap();
        assert_eq!(
            pay,
            "c37809f27c6d7fd1837ad2500a04512b66b34fd793a39a385fade56dca89a4b5"
        );
        let signature = user_signature("9hAb/NEYUlkaMBEsmFgzig==", post_body).unwrap();
        assert_eq!(
            signature,
            "089d9e8dc5d308977360c4b79ec600a93d736802802a807d634192328032f6c7"
        );

        let sign_data = "{\"offerId\":\"123\",\"buyQuantity\":1,\"env\":1,\"currencyType\":\"CNY\",\"productId\":\"small\",\"goodsPrice\":600,\"outTradeNo\":\"abcdef0123456789abcdef0123456789\",\"attach\":\"pack=small&order=abcdef0123456789abcdef0123456789\"}";
        assert_eq!(
            pay_sig("12345", REQUEST_VIRTUAL_PAYMENT, sign_data).unwrap(),
            "ab70f43d857eb190a7aa35965da7929c1e2d43c49b3205777e91fb747832c7e2"
        );
        assert_eq!(
            user_signature("9hAb/NEYUlkaMBEsmFgzig==", sign_data).unwrap(),
            "fba7fdfbd4ad494a9ebd63993df4ef134c35f72daa5bfb45ceb2b1f800e586a8"
        );
    }

    #[test]
    fn message_push_signature_matches_sorted_sha1() {
        let token = "pindou-msg-token";
        let timestamp = "1710000000";
        let nonce = "n0nce";
        assert_eq!(
            message_signature(token, timestamp, nonce),
            "592d0bda4696085f8a53f101525cc56cd784272d"
        );
        assert!(message_signature_matches(
            token,
            timestamp,
            nonce,
            "592D0BDA4696085F8A53F101525CC56CD784272D"
        ));
        assert!(!message_signature_matches(token, timestamp, nonce, "00"));
    }

    #[test]
    fn sign_data_keeps_wechat_field_order_and_default_product_id() {
        let config = live_sandbox();
        let VirtualMode::Live(creds) = virtual_mode(&config) else {
            panic!("expected live sandbox");
        };
        assert_eq!(creds.env, 1);
        assert_eq!(creds.product_id("popular").unwrap(), "popular");
        let sign_data = sign_data_json(
            &creds,
            "small",
            600,
            "abcdef0123456789abcdef0123456789",
            "pack=small&order=abcdef0123456789abcdef0123456789",
        )
        .unwrap();
        assert_eq!(
            sign_data,
            "{\"offerId\":\"offer-test\",\"buyQuantity\":1,\"env\":1,\"currencyType\":\"CNY\",\"productId\":\"small\",\"goodsPrice\":600,\"outTradeNo\":\"abcdef0123456789abcdef0123456789\",\"attach\":\"pack=small&order=abcdef0123456789abcdef0123456789\"}"
        );
        assert!(!sign_data.contains("sandbox-key"));
        assert!(!valid_out_trade_no("_abcdef12"));
        assert!(!valid_out_trade_no("short"));
        assert!(valid_out_trade_no("abcdef0123456789abcdef0123456789"));
    }

    #[test]
    fn env_selects_sandbox_by_default_and_prod_only_when_zero() {
        let mut config = live_sandbox();
        config.wx_virtual_appkey = "prod-key".into();
        match virtual_mode(&config) {
            VirtualMode::Live(creds) => {
                assert_eq!(creds.env, 1);
                assert_eq!(creds.app_key, "sandbox-key");
            }
            _ => panic!("sandbox stays default when both keys exist"),
        }

        config.wx_virtual_env = "0".into();
        match virtual_mode(&config) {
            VirtualMode::Live(creds) => {
                assert_eq!(creds.env, 0);
                assert_eq!(creds.app_key, "prod-key");
            }
            _ => panic!("env 0 uses prod"),
        }

        config.wx_virtual_appkey.clear();
        config.wx_virtual_appkey_prod = "prod-alias".into();
        match virtual_mode(&config) {
            VirtualMode::Live(creds) => assert_eq!(creds.app_key, "prod-alias"),
            _ => panic!("WX_VIRTUAL_APPKEY_PROD is accepted"),
        }

        config.wx_virtual_env.clear();
        config.wx_virtual_appkey_sandbox.clear();
        match virtual_mode(&config) {
            VirtualMode::Misconfigured(reason) => {
                assert!(reason.contains("WX_VIRTUAL_APPKEY_SANDBOX"));
            }
            _ => panic!("unset env does not silently switch to prod"),
        }

        config.wx_virtual_appkey = "one".into();
        config.wx_virtual_appkey_prod = "two".into();
        config.wx_virtual_env = "0".into();
        match virtual_mode(&config) {
            VirtualMode::Misconfigured(reason) => assert!(reason.contains("不一致")),
            _ => panic!("conflicting prod keys"),
        }
    }

    #[test]
    fn product_override_and_unset() {
        assert!(matches!(virtual_mode(&blank_config()), VirtualMode::Unset));
        let mut config = live_sandbox();
        config.wx_virtual_product_small = "sku_small".into();
        let VirtualMode::Live(creds) = virtual_mode(&config) else {
            panic!("live");
        };
        assert_eq!(creds.product_id("small").unwrap(), "sku_small");
        assert_eq!(creds.product_id("large").unwrap(), "large");
    }

    #[test]
    fn deliver_push_json_and_xml() {
        let json = br#"{"Event":"xpay_goods_deliver_notify","OpenId":"o_user","OutTradeNo":"abcdef0123456789abcdef0123456789","Env":1,"GoodsInfo":{"ProductId":"small","Quantity":1,"OrigPrice":600,"ActualPrice":600,"Attach":"pack=small"},"WeChatPayInfo":{"TransactionId":"4200"}}"#;
        match parse_push(json).unwrap() {
            ParsedPush::Deliver(push) => {
                assert_eq!(push.product_id, "small");
                assert_eq!(push.transaction_id, "4200");
                assert_eq!(push.env, 1);
                assert_eq!(push.actual_price, 600);
            }
            _ => panic!("deliver"),
        }
        let xml = r#"<xml><Event><![CDATA[xpay_goods_deliver_notify]]></Event><OpenId><![CDATA[o_user]]></OpenId><OutTradeNo><![CDATA[abcdef0123456789abcdef0123456789]]></OutTradeNo><Env>1</Env><GoodsInfo><ProductId><![CDATA[popular]]></ProductId><Quantity>1</Quantity><OrigPrice>1800</OrigPrice><ActualPrice>1800</ActualPrice><Attach><![CDATA[pack=popular]]></Attach></GoodsInfo></xml>"#;
        match parse_push(xml.as_bytes()).unwrap() {
            ParsedPush::Deliver(push) => {
                assert_eq!(push.product_id, "popular");
                assert_eq!(push.transaction_id, "xpay:abcdef0123456789abcdef0123456789");
            }
            _ => panic!("xml"),
        }
        match parse_push(br#"{"Event":"xpay_refund_notify"}"#).unwrap() {
            ParsedPush::Ignored { event } => assert_eq!(event, "xpay_refund_notify"),
            _ => panic!("ignore"),
        }
        let refund = br#"{"Event":"xpay_refund_notify","OpenId":"o_user","MchOrderId":"abcdef0123456789abcdef0123456789","MchRefundId":"r0123456789abcdef0123456789abc","RefundFee":600,"RetCode":0}"#;
        match parse_push(refund).unwrap() {
            ParsedPush::Refund(push) => {
                assert!(push.success);
                assert_eq!(push.refund_fee, 600);
                assert_eq!(push.out_trade_no, "abcdef0123456789abcdef0123456789");
            }
            _ => panic!("refund"),
        }
        let failed = br#"{"Event":"xpay_refund_notify","MchOrderId":"abcdef0123456789abcdef0123456789","RetCode":1,"RefundFee":600}"#;
        match parse_push(failed).unwrap() {
            ParsedPush::Refund(push) => assert!(!push.success),
            _ => panic!("refund fail"),
        }
    }

    #[test]
    fn refund_body_keeps_field_order_for_pay_sig() {
        let body = refund_body_json(
            "o_user",
            "abcdef0123456789abcdef0123456789",
            "r0123456789abcdef0123456789abc",
            600,
            600,
            "order=abcdef0123456789abcdef0123456789",
            1,
        )
        .unwrap();
        assert_eq!(
            body,
            "{\"openid\":\"o_user\",\"order_id\":\"abcdef0123456789abcdef0123456789\",\"refund_order_id\":\"r0123456789abcdef0123456789abc\",\"left_fee\":600,\"refund_fee\":600,\"biz_meta\":\"order=abcdef0123456789abcdef0123456789\",\"refund_reason\":\"2\",\"req_from\":\"1\",\"env\":1}"
        );
        let sig = pay_sig("12345", REFUND_URI, &body).unwrap();
        assert_eq!(sig.len(), 64);
        assert!(valid_refund_order_id("r0123456789abcdef0123456789abc"));
        assert!(!valid_refund_order_id("short"));
        assert!(!valid_refund_order_id("bad|id-ok"));
    }

    #[test]
    fn query_paid_status_requires_matching_env_type() {
        let body = br#"{"errcode":0,"order":{"status":2,"order_fee":600,"paid_fee":600,"env_type":2,"wxpay_order_id":"4200","wx_order_id":"wx1"}}"#;
        match interpret_query(body, 1, "abcdef0123456789abcdef0123456789").unwrap() {
            QueryOutcome::Paid(facts) => {
                assert_eq!(facts.transaction_id, "4200");
                assert_eq!(facts.order_fee, Some(600));
                assert_eq!(facts.wx_order_id.as_deref(), Some("wx1"));
            }
            _ => panic!("paid"),
        }
        let prod = br#"{"errcode":0,"order":{"status":2,"order_fee":600,"env_type":1}}"#;
        assert!(matches!(
            interpret_query(prod, 1, "abcdef0123456789abcdef0123456789").unwrap(),
            QueryOutcome::NotReady
        ));
        let closed = br#"{"errcode":0,"order":{"status":6,"env_type":2,"order_fee":600}}"#;
        assert!(matches!(
            interpret_query(closed, 1, "abcdef0123456789abcdef0123456789").unwrap(),
            QueryOutcome::ClosedOrRefunded
        ));
    }
}
