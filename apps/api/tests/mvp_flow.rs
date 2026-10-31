//! 核心链路：登录、日领幂等、出稿扣分、广场读写。需要本机 PostgreSQL。

use std::io::Cursor;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn png_base64() -> String {
    let img = RgbaImage::from_fn(24, 24, |x, y| {
        if x < 12 {
            Rgba([0xFF, 0x6B, 0x5B, 255])
        } else if y < 12 {
            Rgba([0x7B, 0xDF, 0xF2, 255])
        } else {
            Rgba([0xFF, 0xD1, 0x66, 255])
        }
    });
    let mut buf = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut buf, ImageFormat::Png)
        .unwrap();
    base64_std(&buf.into_inner())
}

fn base64_std(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | bytes[i + 2] as u32;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push(T[(n & 63) as usize] as char);
        i += 3;
    }
    if i < bytes.len() {
        let rem = bytes.len() - i;
        let mut n = (bytes[i] as u32) << 16;
        if rem == 2 {
            n |= (bytes[i + 1] as u32) << 8;
        }
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if rem == 2 {
            out.push(T[((n >> 6) & 63) as usize] as char);
            out.push('=');
        } else {
            out.push('=');
            out.push('=');
        }
    }
    out
}

async fn app() -> axum::Router {
    let config = pindou_api::Config::from_env();
    let state = pindou_api::connect(&config)
        .await
        .expect("连接 PostgreSQL 失败，请先启动 docker compose");
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

#[tokio::test]
async fn mvp_auth_credits_generate_plaza() {
    let app = app().await;
    let code = format!("mock:it-{}", Uuid::new_v4());

    let (status, body) = call(&app, "GET", "/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");

    let (status, _body) = call(
        &app,
        "POST",
        "/api/v1/auth/wx-login",
        None,
        Some(json!({ "code": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, login) = call(
        &app,
        "POST",
        "/api/v1/auth/wx-login",
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    assert_eq!(login["mode"], "dev");
    let token = login["token"].as_str().unwrap();

    let (status, again) = call(
        &app,
        "POST",
        "/api/v1/auth/wx-login",
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["user"]["id"], login["user"]["id"]);

    let (status, me) = call(&app, "GET", "/api/v1/credits/me", Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["balance"], 0);
    assert_eq!(me["daily_claimed_today"], false);

    let (status, claimed) = call(
        &app,
        "POST",
        "/api/v1/credits/daily-claim",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{claimed}");
    assert_eq!(claimed["balance"], 3);
    assert_eq!(claimed["delta"], 3);
    assert_eq!(claimed["reason"], "daily_claim");

    let (status, me_after) = call(&app, "GET", "/api/v1/credits/me", Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{me_after}");
    assert_eq!(me_after["daily_claimed_today"], true);
    assert_eq!(me_after["balance"], 3);

    let (status, dup) = call(
        &app,
        "POST",
        "/api/v1/credits/daily-claim",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{dup}");
    assert_eq!(dup["error"], "already_claimed");

    let (status, missing) = call(
        &app,
        "POST",
        "/api/v1/drafts/generate",
        Some(token),
        Some(json!({ "board_size": 52, "color_count": 8 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{missing}");
    assert!(
        missing["message"]
            .as_str()
            .unwrap_or("")
            .contains("请先选择图片"),
        "{missing}"
    );

    let (status, bad) = call(
        &app,
        "POST",
        "/api/v1/drafts/generate",
        Some(token),
        Some(json!({
            "image_base64": "not-an-image",
            "board_size": "52x52",
            "color_count": 8
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    let bad_msg = bad["message"].as_str().unwrap_or("");
    assert!(
        bad_msg.contains("无法解析") || bad_msg.contains("图片数据无效"),
        "{bad}"
    );

    let (_status, still) = call(&app, "GET", "/api/v1/credits/me", Some(token), None).await;
    assert_eq!(still["balance"], 3, "失败出稿不应扣分 {still}");

    let (status, generated) = call(
        &app,
        "POST",
        "/api/v1/drafts/generate",
        Some(token),
        Some(json!({
            "image_base64": png_base64(),
            "board_size": 52,
            "color_count": 8,
            "palette_pref": "auto",
            "title": "集成测试稿"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{generated}");
    assert_eq!(generated["status"], "ready");
    assert_eq!(generated["balance"], 2);
    assert!(generated["draft"]["palette"].as_array().unwrap().len() >= 1);
    assert!(generated["draft"]["palette"].as_array().unwrap().len() <= 8);
    assert_eq!(generated["draft"]["peg_count"], 52 * 52);
    let draft_id = generated["draft"]["id"].as_str().unwrap();
    let preview_url = generated["draft"]["preview_url"].as_str().unwrap();
    assert!(preview_url.contains("/files/previews/"));

    let (status, listed) = call(
        &app,
        "GET",
        "/api/v1/drafts?tab=mine&q=%E9%9B%86%E6%88%90",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert!(listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["id"] == draft_id));

    let (status, fav) = call(
        &app,
        "POST",
        &format!("/api/v1/drafts/{draft_id}/favorite"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{fav}");
    assert_eq!(fav["favorited"], true);

    let (status, work) = call(
        &app,
        "POST",
        &format!("/api/v1/drafts/{draft_id}/publish"),
        Some(token),
        Some(json!({ "category": "pet", "caption": "测试发布" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{work}");
    let work_id = work["id"].as_str().unwrap();

    let (status, feed) = call(
        &app,
        "GET",
        "/api/v1/plaza/feed?tab=recommend&limit=20",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{feed}");
    assert!(feed["items"].as_array().unwrap().len() >= 1);

    let (status, liked) = call(
        &app,
        "POST",
        &format!("/api/v1/plaza/works/{work_id}/like"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{liked}");
    assert_eq!(liked["liked"], true);

    let (status, comment) = call(
        &app,
        "POST",
        &format!("/api/v1/plaza/works/{work_id}/comments"),
        Some(token),
        Some(json!({ "content": "色号清楚，准备开拼" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{comment}");
    assert_eq!(comment["content"], "色号清楚，准备开拼");

    let (status, ad) = call(&app, "POST", "/api/v1/credits/ad-reward", Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{ad}");
    assert_eq!(ad["balance"], 3);
    assert_eq!(ad["delta"], 1);

    let (status, order) = call(
        &app,
        "POST",
        "/api/v1/credits/orders",
        Some(token),
        Some(json!({ "pack_code": "small" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{order}");
    let order_id = order["order_id"].as_str().unwrap();
    let (status, paid) = call(
        &app,
        "POST",
        &format!("/api/v1/credits/orders/{order_id}/mock-callback"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{paid}");
    assert_eq!(paid["delta"], 10);
    assert_eq!(paid["balance"], 13);
    let (status, paid_again) = call(
        &app,
        "POST",
        &format!("/api/v1/credits/orders/{order_id}/mock-callback"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{paid_again}");
    assert_eq!(paid_again["delta"], 0);
    assert_eq!(paid_again["balance"], 13);

    let (status, broke) = call(
        &app,
        "POST",
        "/api/v1/drafts/generate",
        Some(token),
        Some(json!({ "board_size": 29, "image_ref": "builtin:cat" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{broke}");
    assert!(broke["message"].as_str().unwrap_or("").contains("52"));

    let broke_code = format!("mock:broke-{}", Uuid::new_v4());
    let (status, broke_login) = call(
        &app,
        "POST",
        "/api/v1/auth/wx-login",
        None,
        Some(json!({ "code": broke_code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{broke_login}");
    let broke_token = broke_login["token"].as_str().unwrap();
    let (status, denied) = call(
        &app,
        "POST",
        "/api/v1/drafts/generate",
        Some(broke_token),
        Some(json!({ "image_ref": "builtin:bunny", "board_size": 52, "color_count": 6 })),
    )
    .await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "{denied}");
    assert_eq!(denied["error"], "insufficient_credits");
    assert!(denied["message"]
        .as_str()
        .unwrap_or("")
        .contains("积分不足"));
}
