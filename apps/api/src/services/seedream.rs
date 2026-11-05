//! 火山方舟 Seedream Flash 图生图。
//!
//! 默认路径：上传原图 → `POST /api/v3/images/generations` → 自研最近邻拼豆。
//! 模型 `doubao-seedream-5-0-flash-260915` 返回约 1K 平涂图。提示词里的点阵说法只是风格，
//! 珠板 52 / 78 / 104 由出稿参数决定，在这一步之后才缩放。
//!
//! 没有 `ARK_API_KEY`，或 `PIPELINE_PROVIDER=mock` 时走确定性本地模拟：
//! 透明底平涂 PNG，不访问网络。阿里云视觉智能密钥不能用于这里。

use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{DynamicImage, ImageFormat, RgbaImage};
use serde_json::{json, Value};

use crate::config::Config;

/// 方舟模型 ID。可用 `ARK_MODEL` 覆盖，默认就是 Flash。
pub const SEEDREAM_MODEL_ID: &str = "doubao-seedream-5-0-flash-260915";

/// 每次图生图都带上的固定提示词。点阵是风格意图，不是输出像素，也不是珠板规格。
pub const SEEDREAM_PROMPT: &str = "保留主体轮廓、核心色彩与标志性特征；转为平涂插画风格；严格按目标珠板点阵意图绘制；去除背景为透明 Alpha；简约色块图标。禁止环境背景、白底填充、写实材质、主体残缺。";

/// 方舟侧的出图规格。约 1024×1024，不是 52×52 / 78×78 / 104×104。
pub const SEEDREAM_IMAGE_SIZE: &str = "1K";

pub const ARK_DEFAULT_BASE: &str = "https://ark.cn-beijing.volces.com";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubjectKind {
    Person,
    Pet,
    Object,
}

impl SubjectKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "" | "person" | "human" | "人像" => Some(Self::Person),
            "pet" | "animal" | "宠物" => Some(Self::Pet),
            "object" | "thing" | "静物" | "物品" => Some(Self::Object),
            other => match other.to_ascii_lowercase().as_str() {
                "person" | "human" => Some(Self::Person),
                "pet" | "animal" => Some(Self::Pet),
                "object" | "thing" => Some(Self::Object),
                _ => None,
            },
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Pet => "pet",
            Self::Object => "object",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Person => "人像",
            Self::Pet => "宠物",
            Self::Object => "静物",
        }
    }
}

#[derive(Debug)]
pub enum SeedreamError {
    Timeout,
    /// `message` 给用户看，已经脱敏。`upstream` 只进运营告警，例如 Seedream code 和截断后的原因。
    Remote {
        message: String,
        retryable: bool,
        upstream: Option<String>,
    },
    Decode,
    Unconfigured,
}

impl SeedreamError {
    /// 服务端已知的上游摘要。用户可见文案不包含这段。
    pub fn upstream_detail(&self) -> Option<String> {
        match self {
            Self::Timeout => Some("timeout".into()),
            Self::Decode => Some("decode".into()),
            Self::Unconfigured => Some("unconfigured".into()),
            Self::Remote { upstream, .. } => upstream.clone(),
        }
    }

    pub fn into_message(self) -> String {
        let text = match self {
            Self::Timeout => "出稿超时了。本次未扣积分，可直接重试。".into(),
            Self::Remote { message, .. } => {
                if message.contains("未扣积分") {
                    message
                } else {
                    format!("{message} 本次未扣积分。")
                }
            }
            Self::Decode => "出稿结果无法解析。本次未扣积分。".into(),
            Self::Unconfigured => "出稿服务未就绪。本次未扣积分。".into(),
        };
        text
    }

    fn retryable(&self) -> bool {
        match self {
            Self::Timeout => true,
            Self::Remote { retryable, .. } => *retryable,
            Self::Decode | Self::Unconfigured => false,
        }
    }
}

pub fn http_client() -> Result<reqwest::Client, SeedreamError> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|_| {
            remote(
                "出稿服务暂时不可用。本次未扣积分。",
                false,
                Some("http-client".into()),
            )
        })
}

pub fn generations_url(base: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if base.ends_with("/images/generations") {
        base.to_string()
    } else if base.ends_with("/api/v3") {
        format!("{base}/images/generations")
    } else {
        format!("{base}/api/v3/images/generations")
    }
}

/// 图生图请求体。`size` 固定为方舟的 1K，不写入珠板规格。
///
/// Flash（`doubao-seedream-5-0-flash-260915`）不接受 `sequential_image_generation`，
/// 带上会整单失败。这里不发送任何 `sequential_*` 字段。
pub fn generation_body(model: &str, image: &str) -> Value {
    json!({
        "model": model,
        "prompt": SEEDREAM_PROMPT,
        "image": image,
        "size": SEEDREAM_IMAGE_SIZE,
        "response_format": "b64_json",
        "watermark": false,
        "stream": false
    })
}

/// 参考图一律内联成 data URL。
///
/// `public_url` 只保留给调用方对照，这里故意不用。方舟机房在国内，生产
/// `PUBLIC_BASE_URL`（例如海外自建域名）和本机 loopback 都可能拉不到图，
/// 交给方舟会返回 `Timeout while down`。调用方在请求前已经读到原图字节。
pub fn reference_image(_public_url: &str, bytes: &[u8]) -> String {
    data_url(bytes)
}

pub async fn run_ark(
    http: &reqwest::Client,
    cfg: &Config,
    image_url: &str,
    bytes: &[u8],
) -> Result<Vec<u8>, SeedreamError> {
    if !cfg.ark_ready() {
        return Err(SeedreamError::Unconfigured);
    }
    let image = reference_image(image_url, bytes);
    let body = generation_body(cfg.ark_model(), &image);
    let url = generations_url(cfg.ark_base_url());
    let mut last = None;
    for attempt in 0..2 {
        match post_generation(http, &url, cfg.ark_api_key.trim(), &body).await {
            Ok(png) => return Ok(png),
            Err(err) if err.retryable() && attempt == 0 => {
                tracing::warn!("方舟出稿将重试一次");
                tokio::time::sleep(Duration::from_millis(800)).await;
                last = Some(err);
            }
            Err(err) => return Err(err),
        }
    }
    Err(last.unwrap_or_else(|| remote("出稿失败，请稍后重试。本次未扣积分。", false, None)))
}

/// 同一张图得到同一张透明底平涂 PNG。尺寸跟原图走，不缩成珠板。
pub fn mock_stylize(bytes: &[u8]) -> Result<Vec<u8>, SeedreamError> {
    let img = image::load_from_memory(bytes).map_err(|_| SeedreamError::Decode)?;
    let mut rgba = img.to_rgba8();
    let w = rgba.width();
    let h = rgba.height();
    if w == 0 || h == 0 {
        return Err(SeedreamError::Decode);
    }
    let erx = 0.38_f32;
    let ery = 0.44_f32;
    let ecx = 0.50_f32;
    let ecy = 0.48_f32;
    for (x, y, pixel) in rgba.enumerate_pixels_mut() {
        let nx = x as f32 / w as f32;
        let ny = y as f32 / h as f32;
        let dx = (nx - ecx) / erx;
        let dy = (ny - ecy) / ery;
        let inside = dx * dx + dy * dy <= 1.0;
        if !inside || pixel[3] == 0 {
            pixel[3] = 0;
            continue;
        }
        let rgb = posterize(saturate([pixel[0], pixel[1], pixel[2]], 1.38), 5);
        pixel[0] = rgb[0];
        pixel[1] = rgb[1];
        pixel[2] = rgb[2];
    }
    encode_png(&rgba)
}

pub fn subject_label(raw: &str) -> &'static str {
    SubjectKind::parse(raw)
        .map(SubjectKind::label)
        .unwrap_or("人像")
}

pub fn pref_label(raw: &str) -> &'static str {
    match raw {
        "warm" => "暖色优先",
        "macaron" => "马卡龙",
        "contrast" => "高对比",
        _ => "自动色盘",
    }
}

pub fn brand_label(raw: &str) -> &'static str {
    crate::services::pixel::Brand::parse(raw)
        .map(|b| b.label())
        .unwrap_or("通用拼豆")
}

pub fn pipeline_tags(pipeline: &str, provider: &str, subject: &str) -> Vec<String> {
    if pipeline != "mvp_plus" {
        return Vec::new();
    }
    let mut tags = Vec::new();
    tags.push(match provider {
        "mock" => "本地色块".to_string(),
        "aliyun" => "历史稿".to_string(),
        _ => "色块整理".to_string(),
    });
    tags.push("拼豆色号".into());
    tags.push(subject_label(subject).into());
    tags
}

pub fn pipeline_lines(
    pipeline: &str,
    provider: &str,
    pref: &str,
    brand: &str,
    board_size: i32,
) -> Vec<String> {
    if pipeline != "mvp_plus" {
        return Vec::new();
    }
    let stylize = match provider {
        "mock" => "本地色块 · 按照片收成色块".to_string(),
        "aliyun" => "历史稿 · 旧版处理已停用".to_string(),
        _ => "色块整理 · 按照片收成色块".to_string(),
    };
    let mut lines = vec![
        stylize,
        format!(
            "最近邻色号 · {board_size}×{board_size} · {}",
            pref_label(pref)
        ),
    ];
    if brand != "generic" && !brand.is_empty() {
        lines.push(format!("色盘品牌 · {}", brand_label(brand)));
    }
    lines
}

async fn post_generation(
    http: &reqwest::Client,
    url: &str,
    api_key: &str,
    body: &Value,
) -> Result<Vec<u8>, SeedreamError> {
    let response = http
        .post(url)
        .bearer_auth(api_key)
        .json(body)
        .send()
        .await
        .map_err(map_http)?;
    let status = response.status();
    let text = response.text().await.map_err(map_http)?;
    if status.as_u16() == 429 || status.is_server_error() {
        tracing::warn!(status = %status, "方舟出稿暂时失败");
        let code = status.as_u16();
        return Err(remote(
            format!("出稿服务繁忙（{code}），请稍后重试。本次未扣积分。"),
            true,
            Some(format!("http {code}")),
        ));
    }
    let value: Value = serde_json::from_str(&text).unwrap_or_else(
        |_| json!({ "error": { "message": text.chars().take(180).collect::<String>() } }),
    );
    if !status.is_success() {
        return Err(remote_from(&value));
    }
    match extract_image(&value)? {
        Extracted::Bytes(raw) => ensure_png(&raw),
        Extracted::Url(image_url) => {
            let downloaded = download_image(http, &image_url).await?;
            ensure_png(&downloaded)
        }
    }
}

enum Extracted {
    Bytes(Vec<u8>),
    Url(String),
}

fn extract_image(value: &Value) -> Result<Extracted, SeedreamError> {
    if let Some(b64) = value
        .pointer("/data/0/b64_json")
        .and_then(|item| item.as_str())
        .filter(|raw| !raw.trim().is_empty())
    {
        return Ok(Extracted::Bytes(decode_b64(b64)?));
    }
    if let Some(url) = value
        .pointer("/data/0/url")
        .and_then(|item| item.as_str())
        .filter(|raw| raw.starts_with("http://") || raw.starts_with("https://"))
    {
        return Ok(Extracted::Url(url.to_string()));
    }
    Err(remote_from(value))
}

fn decode_b64(raw: &str) -> Result<Vec<u8>, SeedreamError> {
    let compact: String = raw.chars().filter(|ch| !ch.is_whitespace()).collect();
    let payload = compact
        .split_once(',')
        .map(|(_, rest)| rest)
        .unwrap_or(compact.as_str());
    STANDARD.decode(payload).map_err(|_| SeedreamError::Decode)
}

async fn download_image(http: &reqwest::Client, url: &str) -> Result<Vec<u8>, SeedreamError> {
    let downloaded = http.get(url).send().await.map_err(map_http)?;
    if !downloaded.status().is_success() {
        return Err(remote(
            "出稿结果下载失败。本次未扣积分。",
            downloaded.status().is_server_error(),
            Some(format!("download {}", downloaded.status().as_u16())),
        ));
    }
    let bytes = downloaded.bytes().await.map_err(map_http)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(remote(
            "出稿结果过大。本次未扣积分。",
            false,
            Some("image-too-large".into()),
        ));
    }
    Ok(bytes.to_vec())
}

fn map_http(err: reqwest::Error) -> SeedreamError {
    if err.is_timeout() {
        SeedreamError::Timeout
    } else {
        tracing::warn!(error = %err, "出稿请求失败");
        remote(
            "出稿失败，请稍后重试。本次未扣积分。",
            false,
            Some("transport".into()),
        )
    }
}

fn remote(message: impl Into<String>, retryable: bool, upstream: Option<String>) -> SeedreamError {
    SeedreamError::Remote {
        message: message.into(),
        retryable,
        upstream: upstream
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty()),
    }
}

fn remote_from(value: &Value) -> SeedreamError {
    let msg = value
        .pointer("/error/message")
        .and_then(|item| item.as_str())
        .or_else(|| value.get("message").and_then(|item| item.as_str()))
        .unwrap_or("");
    let code = value
        .pointer("/error/code")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let reason = clip(msg.trim(), 180);
    let code = clip(code.trim(), 80);
    let summary = match (code.is_empty(), reason.is_empty()) {
        (true, true) => None,
        (false, true) => Some(code.clone()),
        (true, false) => Some(reason),
        (false, false) => Some(format!("{code}: {reason}")),
    };
    let summary = summary
        .map(|text| crate::services::alert::redact(&text, &[]))
        .filter(|text| !text.is_empty());
    if let Some(shown) = summary.as_deref() {
        let short: String = shown.chars().take(80).collect();
        tracing::warn!(code = %code, message = %short, "出稿上游返回错误");
    }
    SeedreamError::Remote {
        message: "出稿失败，请稍后重试。本次未扣积分。".into(),
        retryable: false,
        upstream: summary,
    }
}

fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn data_url(bytes: &[u8]) -> String {
    let mime = if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF") {
        "image/gif"
    } else {
        "image/png"
    };
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

fn ensure_png(bytes: &[u8]) -> Result<Vec<u8>, SeedreamError> {
    let img = image::load_from_memory(bytes).map_err(|_| SeedreamError::Decode)?;
    let rgba = img.to_rgba8();
    if rgba.width() == 0
        || rgba.height() == 0
        || rgba.width() as u64 * rgba.height() as u64 > 16_000_000
    {
        return Err(remote(
            "出稿结果尺寸异常。本次未扣积分。",
            false,
            Some("image-size".into()),
        ));
    }
    encode_png(&rgba)
}

fn saturate(rgb: [u8; 3], factor: f32) -> [u8; 3] {
    let avg = (rgb[0] as f32 + rgb[1] as f32 + rgb[2] as f32) / 3.0;
    rgb.map(|channel| {
        let next = avg + (channel as f32 - avg) * factor;
        next.round().clamp(0.0, 255.0) as u8
    })
}

fn posterize(rgb: [u8; 3], levels: u8) -> [u8; 3] {
    let levels = u16::from(levels.max(2));
    rgb.map(|channel| {
        let bucket = (u16::from(channel) * (levels - 1) + 127) / 255;
        (bucket * 255 / (levels - 1)) as u8
    })
}

fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, SeedreamError> {
    let mut buf = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|_| SeedreamError::Decode)?;
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn png(width: u32, height: u32, paint: impl Fn(u32, u32) -> Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_fn(width, height, paint);
        encode_png(&img).unwrap()
    }

    #[test]
    fn generation_body_uses_flash_and_ignores_board_size() {
        let body = generation_body(SEEDREAM_MODEL_ID, "https://cdn.example.com/a.png");
        assert_eq!(body["model"], SEEDREAM_MODEL_ID);
        assert_eq!(body["prompt"], SEEDREAM_PROMPT);
        assert_eq!(body["size"], "1K");
        assert_eq!(body["response_format"], "b64_json");
        assert_eq!(body["watermark"], false);
        assert!(body.get("sequential_image_generation").is_none());
        assert!(SEEDREAM_PROMPT.contains("目标珠板"));
        let text = body.to_string();
        assert!(!text.contains("sequential_"));
        assert!(!text.contains("52x52"));
        assert!(!text.contains("78x78"));
        assert!(!text.contains("104x104"));
        assert!(!text.contains("SegmentBody"));
        assert!(!text.contains("GenerateHumanAnimeStyle"));
    }

    #[test]
    fn generations_url_points_at_ark_openapi() {
        assert_eq!(
            generations_url(ARK_DEFAULT_BASE),
            "https://ark.cn-beijing.volces.com/api/v3/images/generations"
        );
        assert_eq!(
            generations_url("https://ark.example.com/api/v3/"),
            "https://ark.example.com/api/v3/images/generations"
        );
    }

    #[test]
    fn https_public_url_is_inlined_and_not_handed_to_ark() {
        let bytes = png(2, 2, |_, _| Rgba([1, 2, 3, 255]));
        let overseas =
            "https://api.dongpouu.top/files/uploads/11111111-1111-1111-1111-111111111111.png";
        let image = reference_image(overseas, &bytes);
        assert!(image.starts_with("data:image/png;base64,"), "{image}");
        assert!(!image.contains("https://"), "{image}");
        assert!(!image.contains("dongpouu"), "{image}");
        assert!(!image.contains("/files/"), "{image}");
        let payload = image.split_once(',').expect("data url").1;
        assert_eq!(STANDARD.decode(payload).unwrap(), bytes);

        let local = reference_image("http://127.0.0.1:56778/files/uploads/a.png", &bytes);
        assert!(local.starts_with("data:image/png;base64,"));
        assert!(!local.contains("127.0.0.1"));

        let cdn = reference_image("https://cdn.example.com/cat.png", &bytes);
        assert!(cdn.starts_with("data:image/png;base64,"));
        assert!(!cdn.contains("cdn.example.com"));

        let body = generation_body(SEEDREAM_MODEL_ID, &image);
        assert_eq!(body["image"], image);
        let text = body.to_string();
        assert!(!text.contains("dongpouu"));
        assert!(!text.contains("sequential_"));
    }

    #[test]
    fn jpeg_bytes_use_jpeg_data_url() {
        let bytes = [0xFF, 0xD8, 0xFF, 0xD9];
        let image = reference_image("https://api.example.com/files/uploads/a.jpg", &bytes);
        assert!(image.starts_with("data:image/jpeg;base64,"));
        assert!(!image.contains("http"));
    }

    #[test]
    fn mock_stylize_is_transparent_and_keeps_source_size() {
        let src = png(48, 48, |x, y| {
            let dx = x as i32 - 24;
            let dy = y as i32 - 24;
            if dx * dx + dy * dy < 14 * 14 {
                Rgba([0xFF, 0x6B, 0x5B, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let out = mock_stylize(&src).unwrap();
        let again = mock_stylize(&src).unwrap();
        assert_eq!(out, again);
        assert!(out.starts_with(b"\x89PNG"));
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!((img.width(), img.height()), (48, 48));
        assert_ne!((img.width(), img.height()), (52, 52));
        assert_eq!(img.get_pixel(0, 0)[3], 0);
        assert_eq!(img.get_pixel(24, 24)[3], 255);
    }

    #[test]
    fn b64_generation_payload_decodes_without_network() {
        let png = png(4, 4, |_, _| Rgba([9, 8, 7, 255]));
        let body = json!({
            "data": [{ "b64_json": STANDARD.encode(&png) }]
        });
        match extract_image(&body).unwrap() {
            Extracted::Bytes(raw) => {
                let normalized = ensure_png(&raw).unwrap();
                let img = image::load_from_memory(&normalized).unwrap();
                assert_eq!((img.width(), img.height()), (4, 4));
            }
            Extracted::Url(_) => panic!("expected inline bytes"),
        }
    }

    #[test]
    fn upstream_error_text_stays_off_the_client_message() {
        let err = remote_from(&json!({
            "error": {
                "code": "InvalidParameter",
                "message": "the model doubao-seedream does not exist"
            }
        }));
        assert_eq!(
            err.upstream_detail().as_deref(),
            Some("InvalidParameter: the model doubao-seedream does not exist")
        );
        let text = err.into_message();
        assert_eq!(text, "出稿失败，请稍后重试。本次未扣积分。");
        assert!(!text.to_lowercase().contains("seedream"));
        assert!(!text.contains("doubao"));
        assert!(!text.contains("InvalidParameter"));
    }

    #[test]
    fn policy_violation_alert_keeps_code_off_the_client_message() {
        let err = remote_from(&json!({
            "error": {
                "code": "OutputImageSensitiveContentDetected.PolicyViolation",
                "message": "copyright restrict https://cdn.example.com/files/uploads/ref.png?access_token=supersecrettokenvalue"
            }
        }));
        let upstream = err.upstream_detail().expect("upstream");
        assert!(upstream.contains("OutputImageSensitiveContentDetected.PolicyViolation"));
        assert!(upstream.to_lowercase().contains("copyright"));
        assert!(upstream.contains("uploads/ref.png"), "{upstream}");
        assert!(!upstream.contains("https://"), "{upstream}");
        assert!(!upstream.contains("supersecrettokenvalue"), "{upstream}");
        let user = err.into_message();
        assert_eq!(user, "出稿失败，请稍后重试。本次未扣积分。");
        assert!(!user.contains("PolicyViolation"));
        assert!(!user.to_lowercase().contains("copyright"));

        let mut config = crate::config::Config::from_env();
        config.dingtalk_webhook_url =
            "https://oapi.dingtalk.com/robot/send?access_token=unit-test-token".into();
        config.dingtalk_keyword = "STRATEGY".into();
        config.dingtalk_webhook_secret.clear();
        config.ark_api_key.clear();
        let notice = crate::services::alert::FailureNotice {
            job_id: uuid::Uuid::parse_str("143d6f5b-2a89-4214-8f23-4e78721b2229").unwrap(),
            stage: "seedream".into(),
            user_message: user,
            upstream: Some(upstream),
            provider: "seedream".into(),
            board_size: 52,
            object_key: Some("uploads/ref.png".into()),
            at: chrono::DateTime::parse_from_rfc3339("2026-11-05T01:02:03Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        };
        let outbound =
            crate::services::alert::prepare_outbound(&config, &notice, 1_700_000_000_000)
                .expect("webhook configured");
        let value: serde_json::Value = serde_json::from_str(&outbound.body).unwrap();
        let text = value["markdown"]["text"].as_str().unwrap();
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
        let message_line = text.lines().find(|line| line.contains("message:")).unwrap();
        assert!(!message_line.contains("PolicyViolation"), "{message_line}");
        assert!(
            !outbound.body.contains("unit-test-token"),
            "{}",
            outbound.body
        );
        assert!(!outbound.url.contains("sign="));
    }

    #[test]
    fn tags_name_seedream_or_mock_not_aliyun_matting() {
        let mock = pipeline_tags("mvp_plus", "mock", "pet");
        assert_eq!(mock[0], "本地色块");
        assert!(mock.iter().any(|tag| tag == "拼豆色号"));
        assert!(mock.iter().any(|tag| tag == "宠物"));
        assert!(!mock
            .iter()
            .any(|tag| tag.contains("去背") || tag.contains("卡通")));
        let live = pipeline_lines("mvp_plus", "seedream", "warm", "shop", 78);
        assert!(live[0].contains("色块整理"));
        assert!(!live[0].contains("Seedream"));
        assert!(live[1].contains("78×78"));
        assert!(live.iter().any(|line| line.contains("店内色卡")));
        let shown = format!(
            "{} {}",
            pipeline_tags("mvp_plus", "seedream", "person").join(" "),
            pipeline_lines("mvp_plus", "aliyun", "auto", "generic", 52).join(" ")
        );
        for banned in [
            "AI",
            "Seedream",
            "豆包",
            "方舟",
            "智能",
            "插画",
            "阿里云",
            "大模型",
        ] {
            assert!(!shown.contains(banned), "{banned} in {shown}");
        }
    }
}
