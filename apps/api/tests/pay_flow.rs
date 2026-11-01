//! 微信支付 JSAPI：假微信预下单 + 本地验签回调。不产生真实扣款。

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine;
use http_body_util::BodyExt;
use rsa::pkcs1v15::{Signature, SigningKey, VerifyingKey};
use rsa::pkcs8::{
    DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding,
};
use rsa::signature::{SignatureEncoding, Signer, Verifier};
use rsa::{RsaPrivateKey, RsaPublicKey};
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
    merchant_public: String,
    platform_private: String,
    platform_public: String,
}

fn keys() -> &'static Keys {
    static KEYS: OnceLock<Keys> = OnceLock::new();
    KEYS.get_or_init(|| {
        let (merchant_private, merchant_public) = gen_pem();
        let (platform_private, platform_public) = gen_pem();
        Keys {
            merchant_private,
            merchant_public,
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

fn verify_sha256(public_pem: &str, message: &[u8], signature_b64: &str) -> bool {
    let key = RsaPublicKey::from_public_key_pem(public_pem).unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .unwrap();
    let signature = Signature::try_from(bytes.as_slice()).unwrap();
    VerifyingKey::<Sha256>::new(key)
        .verify(message, &signature)
        .is_ok()
}

#[derive(Clone)]
struct FakeState {
    hit: Arc<Mutex<Option<Hit>>>,
}

struct Hit {
    raw_body: Vec<u8>,
    authorization: String,
    serial_header: String,
}

async fn spawn_fake_wechat() -> (String, Arc<Mutex<Option<Hit>>>) {
    let hit = Arc::new(Mutex::new(None));
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

async fn prepay(State(state): State<FakeState>, headers: HeaderMap, body: Bytes) -> Json<Value> {
    let authorization = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let serial_header = headers
        .get("wechatpay-serial")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    *state.hit.lock().unwrap() = Some(Hit {
        raw_body: body.to_vec(),
        authorization,
        serial_header,
    });
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

fn auth_field<'a>(header: &'a str, key: &str) -> &'a str {
    let pattern = format!("{key}=\"");
    let start = header.find(&pattern).expect(key) + pattern.len();
    let rest = &header[start..];
    let end = rest.find('"').unwrap();
    &rest[..end]
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
async fn jsapi_credits_once_and_rejects_bad_or_unpaid() {
    let (api_base, hit) = spawn_fake_wechat().await;
    let app = live_app(&api_base).await;
    let code = format!("mock:pay-live-{}", Uuid::new_v4());
    let openid = code.clone();
    let token = login(&app, &code).await;
    assert_eq!(balance(&app, &token).await, 0);

    let (status, packs) = call(&app, "GET", "/api/v1/credits/packs", None, None).await;
    assert_eq!(status, StatusCode::OK, "{packs}");
    assert_eq!(packs["pay_mode"], "wechat");
    assert_eq!(packs["mock"], false);

    let (status, order) = call(
        &app,
        "POST",
        "/api/v1/credits/orders",
        Some(&token),
        Some(json!({ "pack_code": "small" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{order}");
    assert_eq!(order["mock"], false);
    assert_eq!(order["status"], "created");
    assert_eq!(order["amount_fen"], 600);
    let order_id = order["order_id"].as_str().unwrap().to_string();
    let out_trade_no = order["out_trade_no"].as_str().unwrap().to_string();
    let params = &order["pay_params"];
    assert_eq!(params["signType"], "RSA");
    assert_eq!(params["package"], "prepay_id=wx_test_prepay_001");
    let pay_message = format!(
        "{APP_ID}\n{}\n{}\n{}\n",
        params["timeStamp"].as_str().unwrap(),
        params["nonceStr"].as_str().unwrap(),
        params["package"].as_str().unwrap()
    );
    assert!(
        verify_sha256(
            &keys().merchant_public,
            pay_message.as_bytes(),
            params["paySign"].as_str().unwrap()
        ),
        "小程序 paySign 与文档签名串不一致"
    );

    let captured = hit.lock().unwrap().take().expect("fake wechat saw prepay");
    assert!(captured
        .authorization
        .starts_with("WECHATPAY2-SHA256-RSA2048 "));
    assert_eq!(auth_field(&captured.authorization, "mchid"), MCH_ID);
    assert_eq!(auth_field(&captured.authorization, "serial_no"), MCH_SERIAL);
    assert_eq!(captured.serial_header, PLATFORM_SERIAL);
    let timestamp = auth_field(&captured.authorization, "timestamp");
    let nonce = auth_field(&captured.authorization, "nonce_str");
    let signature = auth_field(&captured.authorization, "signature").to_string();
    let mut message =
        format!("POST\n/v3/pay/transactions/jsapi\n{timestamp}\n{nonce}\n").into_bytes();
    message.extend_from_slice(&captured.raw_body);
    message.push(b'\n');
    assert!(
        verify_sha256(&keys().merchant_public, &message, &signature),
        "预下单请求签名与文档不一致"
    );
    let sent: Value = serde_json::from_slice(&captured.raw_body).unwrap();
    assert_eq!(sent["appid"], APP_ID);
    assert_eq!(sent["mchid"], MCH_ID);
    assert_eq!(sent["out_trade_no"], out_trade_no);
    assert_eq!(sent["notify_url"], NOTIFY_URL);
    assert_eq!(sent["amount"]["total"], 600);
    assert_eq!(sent["payer"]["openid"], openid);
    assert_eq!(balance(&app, &token).await, 0, "预下单不能加积分");

    let (status, pending) = call(
        &app,
        "GET",
        &format!("/api/v1/credits/orders/{order_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{pending}");
    assert_eq!(pending["status"], "created");
    assert_eq!(pending["balance"], 0);

    let (status, mock) = call(
        &app,
        "POST",
        &format!("/api/v1/credits/orders/{order_id}/mock-callback"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{mock}");
    assert_eq!(balance(&app, &token).await, 0);

    let (body, ts, nonce, sig) = signed_notify(
        &json!({"event_type": "ignored"}),
        "REFUND.SUCCESS",
        "refundnonce1",
    );
    let (status, ignored) = post_notify(&app, &body, &ts, &nonce, &sig).await;
    assert_eq!(status, StatusCode::OK, "{ignored}");
    assert_eq!(balance(&app, &token).await, 0, "非支付成功通知不能加积分");

    let (body, ts, nonce, sig) = signed_notify(
        &trade(&out_trade_no, &openid, 600, "4200000000000000000000099"),
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
    let _ = (body, sig);

    let (body, ts, nonce, sig) = signed_notify(
        &trade(&out_trade_no, &openid, 1, "4200000000000000000000001"),
        "TRANSACTION.SUCCESS",
        "amountonce01",
    );
    let (status, mismatch) = post_notify(&app, &body, &ts, &nonce, &sig).await;
    assert_ne!(status, StatusCode::OK, "{mismatch}");
    assert_eq!(balance(&app, &token).await, 0, "金额不一致不能加积分");
    let (status, still) = call(
        &app,
        "GET",
        &format!("/api/v1/credits/orders/{order_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(still["status"], "created", "{status} {still}");

    let (body, ts, nonce, sig) = signed_notify(
        &trade(
            &out_trade_no,
            "someone-else",
            600,
            "4200000000000000000000002",
        ),
        "TRANSACTION.SUCCESS",
        "openidnonce1",
    );
    let (status, wrong_user) = post_notify(&app, &body, &ts, &nonce, &sig).await;
    assert_ne!(status, StatusCode::OK, "{wrong_user}");
    assert_eq!(balance(&app, &token).await, 0, "openid 不一致不能加积分");

    let (unpaid_status, unpaid) = call(
        &app,
        "POST",
        "/api/v1/credits/orders",
        Some(&token),
        Some(json!({ "pack_code": "popular" })),
    )
    .await;
    assert_eq!(unpaid_status, StatusCode::OK, "{unpaid}");
    assert_eq!(balance(&app, &token).await, 0, "未支付的第二单不能加积分");

    let transaction_id = format!("42{}", &Uuid::new_v4().simple().to_string()[..18]);
    let (body, ts, nonce, sig) = signed_notify(
        &trade(&out_trade_no, &openid, 600, &transaction_id),
        "TRANSACTION.SUCCESS",
        "successnonce",
    );
    let (status, paid) = post_notify(&app, &body, &ts, &nonce, &sig).await;
    assert_eq!(status, StatusCode::OK, "{paid}");
    assert_eq!(paid["code"], "SUCCESS");
    assert_eq!(balance(&app, &token).await, 10);

    let (status, again) = post_notify(&app, &body, &ts, &nonce, &sig).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(balance(&app, &token).await, 10, "重复通知不能再加积分");

    let (left, right) = tokio::join!(
        post_notify(&app, &body, &ts, &nonce, &sig),
        post_notify(&app, &body, &ts, &nonce, &sig),
    );
    assert!(
        left.0.is_success() || right.0.is_success(),
        "{left:?} {right:?}"
    );
    assert_eq!(balance(&app, &token).await, 10);

    let (status, ledger) = call(&app, "GET", "/api/v1/credits/ledger", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{ledger}");
    let recharges: Vec<_> = ledger["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["reason"] == "recharge")
        .collect();
    assert_eq!(recharges.len(), 1);
    assert_eq!(recharges[0]["delta"], 10);

    let (status, done) = call(
        &app,
        "GET",
        &format!("/api/v1/credits/orders/{order_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["status"], "paid");
    assert_eq!(done["balance"], 10);
    assert_eq!(unpaid["status"], "created");
}
