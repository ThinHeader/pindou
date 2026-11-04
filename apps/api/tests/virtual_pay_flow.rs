//! 虚拟支付：本地签名、假微信查单、发货推送。不产生真实扣款，也不写入真实 AppKey。

use std::sync::{Arc, Mutex};

use axum::body::{Body, Bytes};
use axum::extract::{Query, State};
use axum::http::{header, Request, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use hmac::{Hmac, Mac};
use http_body_util::BodyExt;
use serde::Deserialize;
use serde_json::{json, Value};
use sha1::Digest;
use sha2::Sha256;
use tower::ServiceExt;
use uuid::Uuid;

const APP_KEY: &str = "test-sandbox-appkey";
const SESSION: &str = "session-key-do-not-leak";
const MSG_TOKEN: &str = "test-msg-token";
const OFFER_ID: &str = "offer-test";

#[derive(Clone)]
struct Fake {
    status: Arc<Mutex<i64>>,
    fail_query: Arc<Mutex<bool>>,
    provides: Arc<Mutex<u32>>,
    hits: Arc<Mutex<Vec<String>>>,
}

#[derive(Deserialize)]
struct SignedQuery {
    pay_sig: String,
}

async fn spawn_fake() -> (String, Fake) {
    let fake = Fake {
        status: Arc::new(Mutex::new(1)),
        fail_query: Arc::new(Mutex::new(false)),
        provides: Arc::new(Mutex::new(0)),
        hits: Arc::new(Mutex::new(Vec::new())),
    };
    let app = Router::new()
        .route("/cgi-bin/token", get(token))
        .route("/xpay/query_order", post(query_order))
        .route("/xpay/notify_provide_goods", post(provide))
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{addr}");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
        .unwrap();
    let mut ready = false;
    for _ in 0..50 {
        if client
            .get(format!("{base}/cgi-bin/token"))
            .send()
            .await
            .ok()
            .is_some_and(|resp| resp.status().is_success())
        {
            ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(ready, "fake wechat did not start");
    (base, fake)
}

async fn token(State(fake): State<Fake>) -> Json<Value> {
    fake.hits.lock().unwrap().push("token".into());
    Json(json!({"access_token": "tok-test", "expires_in": 7200}))
}

async fn query_order(
    State(fake): State<Fake>,
    Query(query): Query<SignedQuery>,
    body: Bytes,
) -> (StatusCode, Json<Value>) {
    if *fake.fail_query.lock().unwrap() {
        fake.hits.lock().unwrap().push("query-fail".into());
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({})));
    }
    let raw = String::from_utf8_lossy(&body);
    let expect = hmac_hex(APP_KEY, &format!("/xpay/query_order&{raw}"));
    let order_id = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("order_id")
                .and_then(|item| item.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".into());
    fake.hits.lock().unwrap().push(format!(
        "query sig_ok={} status={}",
        query.pay_sig == expect,
        *fake.status.lock().unwrap()
    ));
    if query.pay_sig != expect {
        return (
            StatusCode::OK,
            Json(json!({"errcode": 268490003, "errmsg": "pay_sig"})),
        );
    }
    let status = *fake.status.lock().unwrap();
    (
        StatusCode::OK,
        Json(json!({
            "errcode": 0,
            "errmsg": "ok",
            "order": {
                "status": status,
                "order_fee": 600,
                "paid_fee": 600,
                "env_type": 2,
                "wxpay_order_id": format!("wxpay-{order_id}"),
                "wx_order_id": format!("wx-{order_id}")
            }
        })),
    )
}

async fn provide(
    State(fake): State<Fake>,
    Query(query): Query<SignedQuery>,
    body: Bytes,
) -> (StatusCode, Json<Value>) {
    let raw = String::from_utf8_lossy(&body);
    let expect = hmac_hex(APP_KEY, &format!("/xpay/notify_provide_goods&{raw}"));
    if query.pay_sig != expect {
        return (
            StatusCode::OK,
            Json(json!({"errcode": 268490003, "errmsg": "pay_sig"})),
        );
    }
    *fake.provides.lock().unwrap() += 1;
    (StatusCode::OK, Json(json!({"errcode": 0})))
}

fn hmac_hex(key: &str, message: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).unwrap();
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn msg_sig(timestamp: &str, nonce: &str) -> String {
    let mut parts = [MSG_TOKEN, timestamp, nonce];
    parts.sort_unstable();
    let mut hasher = sha1::Sha1::new();
    hasher.update(parts.concat());
    hex::encode(hasher.finalize())
}

async fn app(api_base: &str) -> (axum::Router, pindou_api::AppState) {
    let mut config = pindou_api::Config::from_env();
    config.pipeline_provider = "mock".into();
    config.force_mock_pay();
    config.wx_appid = "wx_virtual_test".into();
    config.wx_secret = "wx_virtual_secret".into();
    config.wechat_app_id.clear();
    config.wx_virtual_offer_id = OFFER_ID.into();
    config.wx_virtual_appkey_sandbox = APP_KEY.into();
    config.wx_virtual_msg_token = MSG_TOKEN.into();
    config.wx_virtual_api_base = api_base.into();
    let state = pindou_api::connect(&config)
        .await
        .expect("连接 PostgreSQL 失败，请先启动数据库");
    let app = pindou_api::router(state.clone());
    (app, state)
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
    let res = app.clone().oneshot(req).await.unwrap();
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

async fn user_token(state: &pindou_api::AppState, openid: &str, with_session: bool) -> String {
    let user_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO users (openid, nickname)
        VALUES ($1, '拼豆玩家')
        ON CONFLICT (openid) DO UPDATE SET updated_at = now()
        RETURNING id
        "#,
    )
    .bind(openid)
    .fetch_one(&state.db)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO credit_accounts (user_id, balance)
        VALUES ($1, 0)
        ON CONFLICT (user_id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .execute(&state.db)
    .await
    .unwrap();
    if with_session {
        seal_session(state, openid).await;
    }
    pindou_api::issue_token(&state.config.jwt_secret, user_id).unwrap()
}

async fn balance(app: &axum::Router, token: &str) -> i64 {
    let (status, body) = call(app, "GET", "/api/v1/credits/me", Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["balance"].as_i64().unwrap()
}

async fn seal_session(state: &pindou_api::AppState, openid: &str) {
    let enc = pindou_api::seal_session_key_for_tests(&state.config.jwt_secret, SESSION).unwrap();
    assert!(!enc.contains(SESSION));
    sqlx::query("UPDATE users SET wx_session_key_enc = $1 WHERE openid = $2")
        .bind(enc)
        .bind(openid)
        .execute(&state.db)
        .await
        .unwrap();
}

fn deliver_body(openid: &str, out_trade_no: &str, price: i64) -> Value {
    json!({
        "Event": "xpay_goods_deliver_notify",
        "OpenId": openid,
        "OutTradeNo": out_trade_no,
        "Env": 1,
        "GoodsInfo": {
            "ProductId": "small",
            "Quantity": 1,
            "OrigPrice": price,
            "ActualPrice": price,
            "Attach": format!("pack=small&order={out_trade_no}")
        },
        "WeChatPayInfo": { "TransactionId": format!("push-{out_trade_no}") }
    })
}

#[tokio::test]
async fn virtual_order_signs_locally_and_credits_once() {
    let (api_base, fake) = spawn_fake().await;
    let (app, state) = app(&api_base).await;
    let code = format!("mock:virtual-{}", Uuid::new_v4());
    let token = user_token(&state, &code, true).await;
    assert_eq!(balance(&app, &token).await, 0);

    let (status, packs) = call(&app, "GET", "/api/v1/credits/packs", None, None).await;
    assert_eq!(status, StatusCode::OK, "{packs}");
    assert_eq!(packs["pay_mode"], "virtual");
    assert_eq!(packs["mock"], false);

    let (status, denied) = call(
        &app,
        "POST",
        &format!("/api/v1/credits/orders/{}/mock-callback", Uuid::new_v4()),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");

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
    assert!(order.get("pay_params").is_none() || order["pay_params"].is_null());
    let dumped = serde_json::to_string(&order).unwrap();
    assert!(!dumped.contains(APP_KEY));
    assert!(!dumped.contains(SESSION));
    assert!(!dumped.contains("wx_virtual_secret"));

    let params = &order["virtual_payment"];
    assert_eq!(params["mode"], "short_series_goods");
    let sign_data = params["signData"].as_str().unwrap();
    let pay_sig = params["paySig"].as_str().unwrap();
    let signature = params["signature"].as_str().unwrap();
    assert!(pay_sig
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert!(signature
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_eq!(
        pay_sig,
        hmac_hex(APP_KEY, &format!("requestVirtualPayment&{sign_data}"))
    );
    assert_eq!(signature, hmac_hex(SESSION, sign_data));
    let parsed: Value = serde_json::from_str(sign_data).unwrap();
    let out_trade_no = order["out_trade_no"].as_str().unwrap();
    assert_eq!(parsed["offerId"], OFFER_ID);
    assert_eq!(parsed["buyQuantity"], 1);
    assert_eq!(parsed["env"], 1);
    assert_eq!(parsed["currencyType"], "CNY");
    assert_eq!(parsed["productId"], "small");
    assert_eq!(parsed["goodsPrice"], 600);
    assert_eq!(parsed["outTradeNo"], out_trade_no);
    assert_eq!(parsed["attach"], format!("pack=small&order={out_trade_no}"));
    assert!(out_trade_no.len() >= 8 && out_trade_no.len() <= 32);
    assert!(!out_trade_no.starts_with('_'));
    assert_eq!(balance(&app, &token).await, 0);

    let order_id = order["order_id"].as_str().unwrap();
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
    assert_eq!(balance(&app, &token).await, 0);

    *fake.status.lock().unwrap() = 2;
    let mut status = StatusCode::OK;
    let mut paid = Value::Null;
    for _ in 0..5 {
        (status, paid) = call(
            &app,
            "GET",
            &format!("/api/v1/credits/orders/{order_id}"),
            Some(&token),
            None,
        )
        .await;
        if paid["status"] == "paid" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }
    assert_eq!(status, StatusCode::OK, "{paid}");
    assert_eq!(
        paid["status"],
        "paid",
        "{paid} hits={:?}",
        fake.hits.lock().unwrap()
    );
    assert_eq!(paid["balance"], 10);
    assert_eq!(balance(&app, &token).await, 10);
    assert_eq!(*fake.provides.lock().unwrap(), 1);

    let (status, again) = call(
        &app,
        "GET",
        &format!("/api/v1/credits/orders/{order_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["balance"], 10);
    assert_eq!(balance(&app, &token).await, 10);

    let echo = "echo-virtual-ok";
    let ts = "1710000000";
    let nonce = "n0nceVirtual";
    let sig = msg_sig(ts, nonce);
    let (status, verified) = call(
        &app,
        "GET",
        &format!("/api/v1/pay/virtual/notify?signature={sig}&timestamp={ts}&nonce={nonce}&echostr={echo}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{verified}");
    assert_eq!(verified["raw"], echo);
    let (status, bad_echo) = call(
        &app,
        "GET",
        &format!(
            "/api/v1/pay/virtual/notify?signature=00&timestamp={ts}&nonce={nonce}&echostr={echo}"
        ),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{bad_echo}");

    *fake.fail_query.lock().unwrap() = true;
    let (status, second) = call(
        &app,
        "POST",
        "/api/v1/credits/orders",
        Some(&token),
        Some(json!({ "pack_code": "small" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    let second_no = second["out_trade_no"].as_str().unwrap();
    let body = deliver_body(&code, second_no, 600);
    let (status, noted) = call(
        &app,
        "POST",
        &format!("/api/v1/pay/virtual/notify?signature={sig}&timestamp={ts}&nonce={nonce}"),
        None,
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{noted}");
    assert_eq!(noted["ErrCode"], 0);
    assert_eq!(balance(&app, &token).await, 20);

    let (status, replay) = call(
        &app,
        "POST",
        &format!("/api/v1/pay/virtual/notify?signature={sig}&timestamp={ts}&nonce={nonce}"),
        None,
        Some(deliver_body(&code, second_no, 600)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(replay["ErrCode"], 0);
    assert_eq!(balance(&app, &token).await, 20);

    let (status, third) = call(
        &app,
        "POST",
        "/api/v1/credits/orders",
        Some(&token),
        Some(json!({ "pack_code": "small" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{third}");
    let third_no = third["out_trade_no"].as_str().unwrap();
    let (status, rejected) = call(
        &app,
        "POST",
        &format!("/api/v1/pay/virtual/notify?signature=00&timestamp={ts}&nonce={nonce}"),
        None,
        Some(deliver_body(&code, third_no, 600)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rejected}");
    assert_eq!(rejected["ErrCode"], 1);
    assert_eq!(balance(&app, &token).await, 20);

    let (status, ignored) = call(
        &app,
        "POST",
        &format!("/api/v1/pay/virtual/notify?signature={sig}&timestamp={ts}&nonce={nonce}"),
        None,
        Some(json!({"Event": "xpay_refund_notify", "OpenId": code})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ignored}");
    assert_eq!(ignored["ErrCode"], 0);
    assert_eq!(balance(&app, &token).await, 20);

    let bare = format!("mock:virtual-nosession-{}", Uuid::new_v4());
    let bare_token = user_token(&state, &bare, false).await;
    let (status, missing) = call(
        &app,
        "POST",
        "/api/v1/credits/orders",
        Some(&bare_token),
        Some(json!({ "pack_code": "small" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{missing}");
    let missing_text = serde_json::to_string(&missing).unwrap();
    assert!(!missing_text.contains(SESSION));
    assert!(!missing_text.contains(APP_KEY));
    assert_eq!(balance(&app, &bare_token).await, 0);
}

#[tokio::test]
async fn partial_virtual_config_never_grants() {
    let mut config = pindou_api::Config::from_env();
    config.pipeline_provider = "mock".into();
    config.force_mock_pay();
    config.wx_appid = "wx_virtual_test".into();
    config.wx_secret = "wx_virtual_secret".into();
    config.wx_virtual_offer_id = OFFER_ID.into();
    let state = pindou_api::connect(&config)
        .await
        .expect("连接 PostgreSQL 失败，请先启动数据库");
    let app = pindou_api::router(state.clone());
    let (status, packs) = call(&app, "GET", "/api/v1/credits/packs", None, None).await;
    assert_eq!(status, StatusCode::OK, "{packs}");
    assert_eq!(packs["pay_mode"], "misconfigured");
    assert_eq!(packs["mock"], false);
    let code = format!("mock:virtual-partial-{}", Uuid::new_v4());
    let token = user_token(&state, &code, false).await;
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
    assert!(!denied["message"].as_str().unwrap_or("").contains(APP_KEY));
    assert_eq!(balance(&app, &token).await, 0);
}
