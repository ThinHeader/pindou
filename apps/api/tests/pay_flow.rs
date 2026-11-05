//! 商户 JSAPI 不能再给积分充值下单。配齐商户参数时下单返回 503，也不会请求预下单。
//! 回调验签失败不加积分。不产生真实扣款。

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine;
use http_body_util::BodyExt;
use rsa::pkcs1v15::SigningKey;
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::signature::{SignatureEncoding, Signer};
use rsa::RsaPrivateKey;
use serde_json::{json, Value};
use sha2::Sha256;
use tower::ServiceExt;
use uuid::Uuid;

const APP_ID: &str = "wx_test_appid_pindou";
const MCH_ID: &str = "1900000109";
const MCH_SERIAL: &str = "TESTMERCHANTSERIAL0001";
const PLATFORM_SERIAL: &str = "PUB_KEY_ID_TEST_PINDOU";
const API_V3_KEY: &str = "0123456789abcdef0123456789abcdef";
const NOTIFY_URL: &str = "https://pay.example.com/api/v1/pay/wechat/notify";

struct Keys {
    merchant_private: String,
    platform_private: String,
    platform_public: String,
}

fn keys() -> &'static Keys {
    static KEYS: OnceLock<Keys> = OnceLock::new();
    KEYS.get_or_init(|| {
        let (merchant_private, _) = gen_pem();
        let (platform_private, platform_public) = gen_pem();
        Keys {
            merchant_private,
            platform_private,
            platform_public,
        }
    })
}

fn gen_pem() -> (String, String) {
    let mut rng = rand::thread_rng();
    let key = RsaPrivateKey::new(&mut rng, 2048).expect("rsa key");
    let private_pem = key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
    let public_pem = key
        .to_public_key()
        .to_public_key_pem(LineEnding::LF)
        .unwrap();
    (private_pem, public_pem.to_string())
}

fn sign_sha256(private_pem: &str, message: &[u8]) -> String {
    let key = RsaPrivateKey::from_pkcs8_pem(private_pem).unwrap();
    let signing = SigningKey::<Sha256>::new(key);
    base64::engine::general_purpose::STANDARD.encode(signing.sign(message).to_bytes())
}

#[derive(Clone)]
struct FakeState {
    hit: Arc<Mutex<bool>>,
}

async fn spawn_fake_wechat() -> (String, Arc<Mutex<bool>>) {
    let hit = Arc::new(Mutex::new(false));
    let app = Router::new()
        .route("/v3/pay/transactions/jsapi", post(prepay))
        .with_state(FakeState { hit: hit.clone() });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    (format!("http://{addr}"), hit)
}

async fn prepay(State(state): State<FakeState>, _headers: HeaderMap, _body: Bytes) -> Json<Value> {
    *state.hit.lock().unwrap() = true;
    Json(json!({ "prepay_id": "wx_test_prepay_001" }))
}

async fn live_app(api_base: &str) -> axum::Router {
    let mut config = pindou_api::Config::from_env();
    config.pipeline_provider = "mock".into();
    config.force_mock_pay();
    config.wx_appid.clear();
    config.wx_secret.clear();
    let material = keys();
    config.wechat_app_id = APP_ID.into();
    config.wechat_mch_id = MCH_ID.into();
    config.wechat_mch_serial_no = MCH_SERIAL.into();
    config.wechat_mch_private_key = material.merchant_private.clone();
    config.wechat_pay_api_v3_key = API_V3_KEY.into();
    config.wechat_pay_notify_url = NOTIFY_URL.into();
    config.wechat_pay_public_key = material.platform_public.clone();
    config.wechat_pay_public_key_id = PLATFORM_SERIAL.into();
    config.wechat_pay_api_base = api_base.into();
    config.pay_credentials_present = true;
    config.pay_config_error = None;
    let state = pindou_api::connect(&config)
        .await
        .expect("连接 PostgreSQL 失败，请先启动数据库");
    pindou_api::router(state)
}

async fn misconfigured_app() -> axum::Router {
    let mut config = pindou_api::Config::from_env();
    config.pipeline_provider = "mock".into();
    config.force_mock_pay();
    config.wx_appid.clear();
    config.wx_secret.clear();
    config.pay_credentials_present = true;
    config.wechat_mch_id = MCH_ID.into();
    config.pay_config_error = None;
    let state = pindou_api::connect(&config)
        .await
        .expect("连接 PostgreSQL 失败，请先启动数据库");
    pindou_api::router(state)
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let req = if let Some(body) = body {
        builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    };
    read_response(app.clone().oneshot(req).await.unwrap()).await
}

async fn post_notify(
    app: &axum::Router,
    body: &str,
    timestamp: &str,
    nonce: &str,
    signature: &str,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/pay/wechat/notify")
        .header("content-type", "application/json")
        .header("wechatpay-timestamp", timestamp)
        .header("wechatpay-nonce", nonce)
        .header("wechatpay-signature", signature)
        .header("wechatpay-serial", PLATFORM_SERIAL)
        .body(Body::from(body.to_string()))
        .unwrap();
    read_response(app.clone().oneshot(req).await.unwrap()).await
}

async fn read_response(res: axum::http::Response<Body>) -> (StatusCode, Value) {
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes) }))
    };
    (status, value)
}

async fn login(app: &axum::Router, code: &str) -> String {
    let (status, body) = call(
        app,
        "POST",
        "/api/v1/auth/wx-login",
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["token"].as_str().unwrap().to_string()
}

async fn balance(app: &axum::Router, token: &str) -> i64 {
    let (status, body) = call(app, "GET", "/api/v1/credits/me", Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["balance"].as_i64().unwrap()
}

fn signed_notify(
    transaction: &Value,
    event_type: &str,
    aead_nonce: &str,
) -> (String, String, String, String) {
    let plain = serde_json::to_vec(transaction).unwrap();
    let ciphertext =
        pindou_api::seal_aead_for_tests(API_V3_KEY, aead_nonce, "transaction", &plain).unwrap();
    let body = serde_json::to_string(&json!({
        "id": format!("EV-{}", Uuid::new_v4().simple()),
        "create_time": "2026-11-01T12:00:00+08:00",
        "resource_type": "encrypt-resource",
        "event_type": event_type,
        "summary": "支付成功",
        "resource": {
            "original_type": "transaction",
            "algorithm": "AEAD_AES_256_GCM",
            "ciphertext": ciphertext,
            "associated_data": "transaction",
            "nonce": aead_nonce
        }
    }))
    .unwrap();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();
    let nonce = Uuid::new_v4().simple().to_string();
    let message = format!("{timestamp}\n{nonce}\n{body}\n");
    let signature = sign_sha256(&keys().platform_private, message.as_bytes());
    (body, timestamp, nonce, signature)
}

fn trade(out_trade_no: &str, openid: &str, amount: i64, transaction_id: &str) -> Value {
    json!({
        "appid": APP_ID,
        "mchid": MCH_ID,
        "out_trade_no": out_trade_no,
        "transaction_id": transaction_id,
        "trade_type": "JSAPI",
        "trade_state": "SUCCESS",
        "trade_state_desc": "支付成功",
        "success_time": "2026-11-01T12:00:00+08:00",
        "payer": { "openid": openid },
        "amount": {
            "total": amount,
            "payer_total": amount,
            "currency": "CNY",
            "payer_currency": "CNY"
        }
    })
}

#[tokio::test]
async fn partial_credentials_never_grant_credits() {
    let app = misconfigured_app().await;
    let code = format!("mock:pay-partial-{}", Uuid::new_v4());
    let token = login(&app, &code).await;
    let (status, packs) = call(&app, "GET", "/api/v1/credits/packs", None, None).await;
    assert_eq!(status, StatusCode::OK, "{packs}");
    assert_eq!(packs["mock"], false);
    assert_eq!(packs["pay_mode"], "misconfigured");

    let (status, denied) = call(
        &app,
        "POST",
        "/api/v1/credits/orders",
        Some(&token),
        Some(json!({ "pack_code": "small" })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{denied}");
    assert!(denied["message"]
        .as_str()
        .unwrap_or("")
        .contains("配置不完整"));
    assert_eq!(balance(&app, &token).await, 0);

    let (status, mock) = call(
        &app,
        "POST",
        &format!("/api/v1/credits/orders/{}/mock-callback", Uuid::new_v4()),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{mock}");
    assert_eq!(balance(&app, &token).await, 0);
}

#[tokio::test]
async fn merchant_jsapi_cannot_open_a_credit_order() {
    let (api_base, hit) = spawn_fake_wechat().await;
    let app = live_app(&api_base).await;
    let code = format!("mock:pay-live-{}", Uuid::new_v4());
    let token = login(&app, &code).await;
    assert_eq!(balance(&app, &token).await, 0);

    let (status, packs) = call(&app, "GET", "/api/v1/credits/packs", None, None).await;
    assert_eq!(status, StatusCode::OK, "{packs}");
    assert_eq!(packs["pay_mode"], "misconfigured");
    assert_eq!(packs["mock"], false);
    assert_ne!(packs["pay_mode"], "wechat");

    for pack in ["small", "popular", "large"] {
        let (status, denied) = call(
            &app,
            "POST",
            "/api/v1/credits/orders",
            Some(&token),
            Some(json!({ "pack_code": pack })),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{pack} {denied}");
        let message = denied["message"].as_str().unwrap_or("");
        assert!(message.contains("虚拟支付"), "{message}");
        assert!(denied.get("pay_params").is_none() || denied["pay_params"].is_null());
        assert!(denied.get("virtual_payment").is_none() || denied["virtual_payment"].is_null());
    }
    assert!(!*hit.lock().unwrap(), "充值不能请求商户 JSAPI 预下单");
    assert_eq!(balance(&app, &token).await, 0);

    let (status, mock) = call(
        &app,
        "POST",
        &format!("/api/v1/credits/orders/{}/mock-callback", Uuid::new_v4()),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{mock}");
    assert_eq!(balance(&app, &token).await, 0);

    let (body, ts, nonce, _sig) = signed_notify(
        &trade(&code, &code, 600, "4200000000000000000000099"),
        "TRANSACTION.SUCCESS",
        "badsignonce1",
    );
    let (status, rejected) = post_notify(
        &app,
        &body,
        &ts,
        &nonce,
        &sign_sha256(&keys().platform_private, b"nope"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{rejected}");
    assert_eq!(rejected["code"], "FAIL");
    assert_eq!(balance(&app, &token).await, 0, "坏签名不能加积分");

    let (body, ts, nonce, sig) = signed_notify(
        &trade(&code, &code, 600, "4200000000000000000000001"),
        "TRANSACTION.SUCCESS",
        "missingordr1",
    );
    let (status, missing) = post_notify(&app, &body, &ts, &nonce, &sig).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
    assert_eq!(
        balance(&app, &token).await,
        0,
        "没有 JSAPI 订单时通知不能加积分"
    );
}
