//! 核心链路：登录、日领幂等、出稿扣分。作品发布与广场互动应返回 410。需要本机 PostgreSQL。

use std::io::Cursor;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn png_bytes() -> Vec<u8> {
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
    buf.into_inner()
}

fn png_base64() -> String {
    base64_std(&png_bytes())
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
    app_configured(|_| {}).await
}

async fn app_configured(edit: impl FnOnce(&mut pindou_api::Config)) -> axum::Router {
    let mut config = pindou_api::Config::from_env();
    // 集成测试不访问方舟，即使本机配了密钥也走模拟管线。
    config.pipeline_provider = "mock".into();
    config.ark_api_key.clear();
    // 即使本机 .env 配了商户参数，这条链路也只测开发示意充值。
    config.force_mock_pay();
    edit(&mut config);
    let state = pindou_api::connect(&config)
        .await
        .expect("连接 PostgreSQL 失败，请先启动数据库");
    pindou_api::router(state)
}

/// 出稿改为异步任务：提交后轮询到 done。失败会把最后一帧打出来。
async fn generate_done(app: &axum::Router, token: &str, body: Value) -> Value {
    let (status, started) = call(
        app,
        "POST",
        "/api/v1/drafts/generate",
        Some(token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let job_id = started["job_id"].as_str().expect("job_id").to_string();
    let mut last = started;
    for _ in 0..80 {
        let st = last["status"].as_str().unwrap_or("");
        if st == "done" || st == "failed" || st == "cancelled" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        let (status, job) = call(
            app,
            "GET",
            &format!("/api/v1/jobs/{job_id}"),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{job}");
        last = job;
    }
    assert_eq!(last["status"], "done", "{last}");
    assert_eq!(last["charged"], true, "{last}");
    last
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

    let generated = generate_done(
        &app,
        token,
        json!({
            "image_base64": png_base64(),
            "board_size": 52,
            "color_count": 8,
            "palette_pref": "auto",
            "title": "集成测试稿"
        }),
    )
    .await;
    assert_eq!(generated["balance"], 2);
    assert_eq!(generated["provider"], "mock");
    assert_eq!(generated["draft"]["pipeline"], "mvp_plus");
    assert!(generated["original_url"]
        .as_str()
        .unwrap()
        .contains("/files/"));
    assert!(generated["matted_url"].is_null(), "{generated}");
    assert!(generated["cartoon_url"]
        .as_str()
        .unwrap()
        .contains("/files/"));
    let stage_ids: Vec<&str> = generated["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|stage| stage["id"].as_str().unwrap())
        .collect();
    assert_eq!(stage_ids, ["upload", "seedream", "pixel"]);
    assert_eq!(generated["draft"]["board_size"], 52);
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

    let (status, closed) = call(
        &app,
        "POST",
        &format!("/api/v1/drafts/{draft_id}/publish"),
        Some(token),
        Some(json!({ "category": "pet", "caption": "测试发布" })),
    )
    .await;
    assert_eq!(status, StatusCode::GONE, "{closed}");
    assert_eq!(closed["error"], "gone");

    let (status, feed) = call(
        &app,
        "GET",
        "/api/v1/plaza/feed?tab=recommend&limit=20",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::GONE, "{feed}");
    assert_eq!(feed["error"], "gone");

    let (status, liked) = call(
        &app,
        "POST",
        "/api/v1/plaza/works/00000000-0000-0000-0000-000000000000/like",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::GONE, "{liked}");

    let (status, comment) = call(
        &app,
        "POST",
        "/api/v1/plaza/works/00000000-0000-0000-0000-000000000000/comments",
        Some(token),
        Some(json!({ "content": "色号清楚，准备开拼" })),
    )
    .await;
    assert_eq!(status, StatusCode::GONE, "{comment}");
    assert_eq!(comment["error"], "gone");

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
    assert_eq!(order["mock"], true);
    assert!(order.get("pay_params").is_none());
    assert_eq!(order["status"], "created");
    let order_id = order["order_id"].as_str().unwrap();
    let (status, pending) = call(
        &app,
        "GET",
        &format!("/api/v1/credits/orders/{order_id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{pending}");
    assert_eq!(pending["status"], "created");
    let (status, before_pay) = call(&app, "GET", "/api/v1/credits/me", Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{before_pay}");
    assert_eq!(before_pay["balance"], 3, "未支付不能加积分");
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

#[tokio::test]
async fn draft_title_rename_and_delete() {
    let app = app().await;
    let code = format!("mock:title-{}", Uuid::new_v4());
    let (status, login) = call(
        &app,
        "POST",
        "/api/v1/auth/wx-login",
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    let token = login["token"].as_str().unwrap();

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

    let long = "豆".repeat(41);
    let (status, too_long) = call(
        &app,
        "POST",
        "/api/v1/drafts/generate",
        Some(token),
        Some(json!({
            "image_base64": png_base64(),
            "board_size": 52,
            "color_count": 4,
            "title": long
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{too_long}");
    assert!(too_long["message"].as_str().unwrap_or("").contains("40"));
    let (status, balance) = call(&app, "GET", "/api/v1/credits/me", Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{balance}");
    assert_eq!(balance["balance"], 3, "非法标题不应扣分");

    let unnamed = generate_done(
        &app,
        token,
        json!({
            "image_base64": png_base64(),
            "board_size": 52,
            "color_count": 4,
            "title": "   "
        }),
    )
    .await;
    assert_eq!(unnamed["draft"]["title"], "未命名草稿");
    assert_eq!(unnamed["balance"], 2);
    let draft_id = unnamed["draft"]["id"].as_str().unwrap();

    let (status, renamed) = call(
        &app,
        "PUT",
        &format!("/api/v1/drafts/{draft_id}"),
        Some(token),
        Some(json!({ "title": "  窗边的猫  " })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert_eq!(renamed["title"], "窗边的猫");

    let (status, blank) = call(
        &app,
        "PATCH",
        &format!("/api/v1/drafts/{draft_id}"),
        Some(token),
        Some(json!({ "title": "  " })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{blank}");
    assert!(blank["message"].as_str().unwrap_or("").contains("请填写"));

    let (status, still) = call(
        &app,
        "GET",
        &format!("/api/v1/drafts/{draft_id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{still}");
    assert_eq!(still["title"], "窗边的猫");

    let other_code = format!("mock:other-{}", Uuid::new_v4());
    let (status, other_login) = call(
        &app,
        "POST",
        "/api/v1/auth/wx-login",
        None,
        Some(json!({ "code": other_code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{other_login}");
    let other = other_login["token"].as_str().unwrap();
    let (status, forbidden) = call(
        &app,
        "PUT",
        &format!("/api/v1/drafts/{draft_id}"),
        Some(other),
        Some(json!({ "title": "别人的稿" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{forbidden}");
    let (status, forbidden_del) = call(
        &app,
        "DELETE",
        &format!("/api/v1/drafts/{draft_id}"),
        Some(other),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{forbidden_del}");

    let (status, missing) = call(
        &app,
        "DELETE",
        &format!("/api/v1/drafts/{}", Uuid::new_v4()),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");

    let (status, official) = call(
        &app,
        "GET",
        "/api/v1/drafts?tab=official",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{official}");
    let official_id = official["items"][0]["id"].as_str().unwrap();
    let (status, official_del) = call(
        &app,
        "DELETE",
        &format!("/api/v1/drafts/{official_id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{official_del}");

    let (status, closed) = call(
        &app,
        "POST",
        &format!("/api/v1/drafts/{draft_id}/publish"),
        Some(token),
        Some(json!({ "title": "窗边的猫", "category": "pet", "caption": "命名发布" })),
    )
    .await;
    assert_eq!(status, StatusCode::GONE, "{closed}");
    assert_eq!(closed["error"], "gone");

    let (status, named_draft) = call(
        &app,
        "GET",
        &format!("/api/v1/drafts/{draft_id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{named_draft}");
    assert_eq!(named_draft["title"], "窗边的猫");
    assert!(named_draft["work_id"].is_null());

    let (status, removed) = call(
        &app,
        "DELETE",
        &format!("/api/v1/drafts/{draft_id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{removed}");
    assert_eq!(removed["deleted"], true);
    assert_eq!(removed["work_deleted"], false);
    let (status, gone_draft) = call(
        &app,
        "GET",
        &format!("/api/v1/drafts/{draft_id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{gone_draft}");
    let (status, gone_work) = call(
        &app,
        "GET",
        "/api/v1/plaza/works/00000000-0000-0000-0000-000000000000",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::GONE, "{gone_work}");

    let second = generate_done(
        &app,
        token,
        json!({
            "image_base64": png_base64(),
            "board_size": 52,
            "color_count": 4,
            "title": "  第二张  "
        }),
    )
    .await;
    assert_eq!(second["draft"]["title"], "第二张");
    assert_eq!(second["balance"], 1);
    let second_id = second["draft"]["id"].as_str().unwrap();

    let (status, dropped) = call(
        &app,
        "DELETE",
        &format!("/api/v1/drafts/{second_id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{dropped}");
    assert_eq!(dropped["work_deleted"], false);

    let builtin = generate_done(
        &app,
        token,
        json!({ "image_ref": "builtin:cat", "board_size": 52, "color_count": 6 }),
    )
    .await;
    assert_eq!(builtin["draft"]["title"], "小橘猫吐舌");
    assert_eq!(builtin["balance"], 0);
    let builtin_id = builtin["draft"]["id"].as_str().unwrap();
    let (status, only_draft) = call(
        &app,
        "DELETE",
        &format!("/api/v1/drafts/{builtin_id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{only_draft}");
    assert_eq!(only_draft["deleted"], true);
    assert_eq!(only_draft["work_deleted"], false);
}

async fn get_bytes(app: &axum::Router, uri: &str) -> (StatusCode, Vec<u8>) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec())
}

#[tokio::test]
async fn mvp_plus_mock_pipeline_tags_and_board_sizes() {
    let app = app().await;
    let code = format!("mock:pipe-{}", Uuid::new_v4());
    let (status, login) = call(
        &app,
        "POST",
        "/api/v1/auth/wx-login",
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    let token = login["token"].as_str().unwrap();
    let (status, claimed) = call(
        &app,
        "POST",
        "/api/v1/credits/daily-claim",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{claimed}");

    let job = generate_done(
        &app,
        token,
        json!({
            "image_base64": png_base64(),
            "board_size": 52,
            "color_count": 8,
            "palette_pref": "warm",
            "remove_bg": true,
            "cartoon_strength": "strong",
            "subject_type": "pet",
            "palette_brand": "shop",
            "title": "管线稿"
        }),
    )
    .await;
    assert_eq!(job["provider"], "mock");
    assert_eq!(job["balance"], 2);
    assert_eq!(job["charged"], true);
    let tags = job["draft"]["pipeline_tags"].as_array().unwrap();
    let tag_text: Vec<&str> = tags.iter().filter_map(|item| item.as_str()).collect();
    assert!(tag_text.contains(&"开发模拟出稿"), "{tag_text:?}");
    assert!(tag_text.contains(&"拼豆量化"), "{tag_text:?}");
    assert!(tag_text.contains(&"宠物"), "{tag_text:?}");
    assert!(
        !tag_text
            .iter()
            .any(|tag| tag.contains("去背") || tag.contains("卡通")),
        "{tag_text:?}"
    );
    let lines = job["draft"]["pipeline_lines"].as_array().unwrap();
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("开发模拟出稿")));
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("52×52")));
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("店内色卡")));
    let shop = ["A1", "B3", "C2", "D1", "K9", "E4", "E7", "W0", "F1", "H1"];
    for swatch in job["draft"]["palette"].as_array().unwrap() {
        let code = swatch["code"].as_str().unwrap();
        assert!(shop.contains(&code), "{code}");
    }
    let cartoon_url = job["cartoon_url"].as_str().unwrap();
    let path = cartoon_url.split("/files/").nth(1).unwrap();
    let (status, bytes) = get_bytes(&app, &format!("/files/{path}")).await;
    assert_eq!(status, StatusCode::OK, "{cartoon_url}");
    assert!(bytes.starts_with(b"\x89PNG"));
    let intermediate = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!((intermediate.width(), intermediate.height()), (24, 24));
    assert_eq!(intermediate.get_pixel(0, 0)[3], 0);
    assert_ne!((intermediate.width(), intermediate.height()), (52, 52));

    let wide = generate_done(
        &app,
        token,
        json!({
            "image_base64": png_base64(),
            "board_size": 78,
            "color_count": 8
        }),
    )
    .await;
    assert_eq!(wide["draft"]["board_size"], 78);
    assert_eq!(wide["draft"]["peg_count"], 78 * 78);
    assert_eq!(wide["balance"], 1);
    assert!(wide["draft"]["pipeline_lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("78×78")));

    let plain = generate_done(
        &app,
        token,
        json!({
            "image_ref": "builtin:bear",
            "board_size": 104,
            "color_count": 6,
            "remove_bg": false,
            "cartoon_strength": "light",
            "subject_type": "object"
        }),
    )
    .await;
    assert!(plain["matted_url"].is_null(), "{plain}");
    assert!(plain["cartoon_url"].as_str().is_some());
    assert_eq!(plain["draft"]["board_size"], 104);
    assert_eq!(plain["draft"]["peg_count"], 104 * 104);
    assert_eq!(plain["balance"], 0);
    let states: Vec<&str> = plain["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|stage| stage["state"].as_str().unwrap())
        .collect();
    assert_eq!(states, ["done", "done", "done"], "{states:?}");

    let job_id = plain["job_id"].as_str().unwrap();
    let other = format!("mock:pipe-other-{}", Uuid::new_v4());
    let (status, other_login) = call(
        &app,
        "POST",
        "/api/v1/auth/wx-login",
        None,
        Some(json!({ "code": other })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{other_login}");
    let other_token = other_login["token"].as_str().unwrap();
    let (status, hidden) = call(
        &app,
        "GET",
        &format!("/api/v1/jobs/{job_id}"),
        Some(other_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{hidden}");

    let (status, missing) = call(
        &app,
        "POST",
        &format!("/api/v1/jobs/{}/cancel", Uuid::new_v4()),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");

    let (status, kept) = call(
        &app,
        "POST",
        &format!("/api/v1/jobs/{job_id}/cancel"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{kept}");
    assert_eq!(
        kept["status"], "done",
        "已完成的任务取消后仍是完成态 {kept}"
    );
    assert_eq!(kept["charged"], true);
}

#[tokio::test]
async fn provider_switch_charges_only_when_mock_succeeds() {
    for provider in ["seedream", "ark", "aliyun"] {
        let app = app_configured(|config| {
            config.pipeline_provider = provider.into();
        })
        .await;
        let code = format!("mock:switch-{provider}-{}", Uuid::new_v4());
        let (status, login) = call(
            &app,
            "POST",
            "/api/v1/auth/wx-login",
            None,
            Some(json!({ "code": code })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{login}");
        let token = login["token"].as_str().unwrap();
        let (status, claimed) = call(
            &app,
            "POST",
            "/api/v1/credits/daily-claim",
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{claimed}");
        let (status, denied) = call(
            &app,
            "POST",
            "/api/v1/drafts/generate",
            Some(token),
            Some(json!({
                "image_base64": png_base64(),
                "board_size": 78,
                "color_count": 8
            })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{provider} {denied}");
        let message = denied["message"].as_str().unwrap_or("");
        assert!(message.contains("ARK_API_KEY"), "{provider} {denied}");
        if provider == "aliyun" {
            assert!(message.contains("视觉智能"), "{denied}");
        }
        let (status, still) = call(&app, "GET", "/api/v1/credits/me", Some(token), None).await;
        assert_eq!(status, StatusCode::OK, "{still}");
        assert_eq!(still["balance"], 3, "{provider} 失败不应扣分 {still}");
    }

    let app = app_configured(|config| {
        config.pipeline_provider = "auto".into();
    })
    .await;
    let code = format!("mock:switch-auto-{}", Uuid::new_v4());
    let (status, login) = call(
        &app,
        "POST",
        "/api/v1/auth/wx-login",
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    let token = login["token"].as_str().unwrap();
    let (status, claimed) = call(
        &app,
        "POST",
        "/api/v1/credits/daily-claim",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{claimed}");
    let generated = generate_done(
        &app,
        token,
        json!({
            "image_base64": png_base64(),
            "board_size": 104,
            "color_count": 8
        }),
    )
    .await;
    assert_eq!(generated["provider"], "mock");
    assert_eq!(generated["charged"], true);
    assert_eq!(generated["balance"], 2);
    assert_eq!(generated["draft"]["board_size"], 104);
    assert_eq!(generated["draft"]["peg_count"], 104 * 104);
}

async fn post_profile(
    app: &axum::Router,
    token: &str,
    nickname: &str,
    png: &[u8],
) -> (StatusCode, Value) {
    let boundary = "----pindouprofile";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"nickname\"\r\n\r\n{nickname}\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"avatar.png\"\r\nContent-Type: image/png\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(png);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/me/profile")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes) }));
    (status, value)
}

#[tokio::test]
async fn profile_avatar_and_nickname_roundtrip() {
    let app = app().await;
    let code = format!("mock:profile-{}", Uuid::new_v4());
    let (status, login) = call(
        &app,
        "POST",
        "/api/v1/auth/wx-login",
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    assert_eq!(login["user"]["nickname"], "拼豆玩家");
    assert!(login["user"]["avatar_url"].is_null());
    let token = login["token"].as_str().unwrap();

    let (status, denied) = call(
        &app,
        "PUT",
        "/api/v1/me",
        None,
        Some(json!({ "nickname": "豆" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{denied}");

    let (status, empty) = call(&app, "PUT", "/api/v1/me", Some(token), Some(json!({}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{empty}");

    let (status, saved) = post_profile(&app, token, "豆豆手作", &png_bytes()).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["user"]["nickname"], "豆豆手作");
    let avatar = saved["user"]["avatar_url"]
        .as_str()
        .expect("avatar url")
        .to_string();
    assert!(avatar.contains("/files/uploads/"), "{avatar}");
    assert!(avatar.starts_with("http"), "{avatar}");

    let (status, me) = call(&app, "GET", "/api/v1/me", Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["user"]["nickname"], "豆豆手作");
    assert_eq!(me["user"]["avatar_url"], avatar);

    let (status, renamed) = call(
        &app,
        "PUT",
        "/api/v1/me",
        Some(token),
        Some(json!({ "nickname": "小豆" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert_eq!(renamed["user"]["nickname"], "小豆");
    assert_eq!(renamed["user"]["avatar_url"], avatar);

    let (status, again) = call(
        &app,
        "POST",
        "/api/v1/auth/wx-login",
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["user"]["nickname"], "小豆");
    assert_eq!(again["user"]["avatar_url"], avatar);

    let (status, feed) = call(
        &app,
        "GET",
        "/api/v1/plaza/feed?tab=recommend&limit=20",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::GONE, "{feed}");
    assert_eq!(feed["error"], "gone");
}

#[tokio::test]
async fn profile_put_json_saves_chinese_nickname_and_avatar_base64() {
    let app = app().await;
    let code = format!("mock:profile-json-{}", Uuid::new_v4());
    let (status, login) = call(
        &app,
        "POST",
        "/api/v1/auth/wx-login",
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    assert_eq!(login["user"]["nickname"], "拼豆玩家");
    assert!(login["user"]["avatar_url"].is_null());
    let token = login["token"].as_str().unwrap();

    let (status, saved) = call(
        &app,
        "PUT",
        "/api/v1/me",
        Some(token),
        Some(json!({ "nickname": "肚圆", "avatar_base64": png_base64() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["user"]["nickname"], "肚圆");
    let avatar = saved["user"]["avatar_url"]
        .as_str()
        .expect("avatar url")
        .to_string();
    assert!(avatar.contains("/files/uploads/"), "{avatar}");

    let (status, me) = call(&app, "GET", "/api/v1/me", Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["user"]["nickname"], "肚圆");
    assert_eq!(me["user"]["avatar_url"], avatar);

    let (status, renamed) = call(
        &app,
        "PUT",
        "/api/v1/me",
        Some(token),
        Some(json!({ "nickname": "小圆" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert_eq!(renamed["user"]["nickname"], "小圆");
    assert_eq!(renamed["user"]["avatar_url"], avatar);

    let (status, bad) = call(
        &app,
        "PUT",
        "/api/v1/me",
        Some(token),
        Some(json!({ "nickname": "小圆", "avatar_base64": "not-an-image" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    assert_eq!(bad["error"], "bad_request");
}
