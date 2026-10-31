//! 去背与卡通。
//!
//! 配了阿里云密钥时走视觉智能开放平台：
//! - 人像：imageseg `SegmentBody`（人体分割）
//! - 宠物 / 静物：imageseg `SegmentCommonImage`（通用分割）
//! - 卡通：imageenhan `GenerateHumanAnimeStyle`（人物动漫化）
//!
//! 密钥缺失或 `PIPELINE_PROVIDER=mock` 时走确定性本地模拟：
//! 透明 PNG + 色阶卡通，不调用网络，也不用扩散模型产珠图。

use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use hmac::{Hmac, Mac};
use image::{DynamicImage, ImageFormat, RgbaImage};
use serde_json::Value;
use sha1::Sha1;

use crate::config::Config;

type HmacSha1 = Hmac<Sha1>;

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

    /// 人像走人体分割，宠物和静物走通用分割。
    pub fn segment_action(self) -> &'static str {
        match self {
            Self::Person => "SegmentBody",
            Self::Pet | Self::Object => "SegmentCommonImage",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CartoonStrength {
    Light,
    Standard,
    Strong,
}

impl CartoonStrength {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "" | "standard" | "std" | "标准" => Some(Self::Standard),
            "light" | "轻" => Some(Self::Light),
            "strong" | "强" => Some(Self::Strong),
            other => match other.to_ascii_lowercase().as_str() {
                "standard" | "std" => Some(Self::Standard),
                "light" => Some(Self::Light),
                "strong" => Some(Self::Strong),
                _ => None,
            },
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Standard => "standard",
            Self::Strong => "strong",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "轻",
            Self::Standard => "标准",
            Self::Strong => "强",
        }
    }

    /// 人物动漫化 AlgoType。轻 / 标准 / 强对应手绘、二次元、3D。
    pub fn algo_type(self) -> &'static str {
        match self {
            Self::Light => "handdrawn",
            Self::Standard => "anime",
            Self::Strong => "3d",
        }
    }
}

#[derive(Debug)]
pub enum VisionError {
    Timeout,
    Remote(String),
    Decode,
    Unconfigured,
}

impl VisionError {
    pub fn into_message(self, stage: &str) -> String {
        match self {
            Self::Timeout => match stage {
                "matting" => "智能去背超时了。本次未扣积分，可直接重试。".into(),
                "cartoon" => "卡通处理超时了。本次未扣积分，可换参数或直接重试。".into(),
                _ => "处理超时了。本次未扣积分。".into(),
            },
            Self::Remote(msg) => {
                if msg.contains("未扣积分") {
                    msg
                } else {
                    format!("{msg} 本次未扣积分。")
                }
            }
            Self::Decode => "这一步返回的图片无法解析。本次未扣积分。".into(),
            Self::Unconfigured => "未配置阿里云访问密钥。本次未扣积分。".into(),
        }
    }
}

pub fn http_client() -> Result<reqwest::Client, VisionError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(40))
        .build()
        .map_err(|_| VisionError::Remote("无法发起视觉服务请求".into()))
}

pub async fn run_matting(
    http: &reqwest::Client,
    cfg: &Config,
    subject: SubjectKind,
    image_url: &str,
    bytes: &[u8],
) -> Result<Vec<u8>, VisionError> {
    if cfg.use_mock_pipeline() {
        return mock_matte(bytes, subject);
    }
    if !cfg.aliyun_ready() {
        return Err(VisionError::Unconfigured);
    }
    aliyun_image(
        http,
        &cfg.imageseg_host(),
        "2019-12-30",
        subject.segment_action(),
        cfg.aliyun_region_id(),
        cfg.aliyun_access_key_id.trim(),
        cfg.aliyun_access_key_secret.trim(),
        &[("ImageURL", image_url)],
        "智能去背",
    )
    .await
}

pub async fn run_cartoon(
    http: &reqwest::Client,
    cfg: &Config,
    strength: CartoonStrength,
    image_url: &str,
    bytes: &[u8],
) -> Result<Vec<u8>, VisionError> {
    if cfg.use_mock_pipeline() {
        return mock_cartoon(bytes, strength);
    }
    if !cfg.aliyun_ready() {
        return Err(VisionError::Unconfigured);
    }
    aliyun_image(
        http,
        &cfg.imageenhan_host(),
        "2019-09-30",
        "GenerateHumanAnimeStyle",
        cfg.aliyun_region_id(),
        cfg.aliyun_access_key_id.trim(),
        cfg.aliyun_access_key_secret.trim(),
        &[("ImageURL", image_url), ("AlgoType", strength.algo_type())],
        "人物动漫化",
    )
    .await
}

/// 按角落背景色把主体以外抠成透明 PNG。同一张图、同一主体类型结果固定。
pub fn mock_matte(bytes: &[u8], subject: SubjectKind) -> Result<Vec<u8>, VisionError> {
    let img = image::load_from_memory(bytes).map_err(|_| VisionError::Decode)?;
    let rgba = img.to_rgba8();
    let w = rgba.width();
    let h = rgba.height();
    if w == 0 || h == 0 {
        return Err(VisionError::Decode);
    }
    let bg = corner_bg(&rgba);
    let (erx, ery, ecx, ecy) = match subject {
        SubjectKind::Person => (0.34_f32, 0.42, 0.50, 0.46),
        SubjectKind::Pet => (0.40, 0.40, 0.50, 0.52),
        SubjectKind::Object => (0.30, 0.30, 0.50, 0.50),
    };
    let mut out = rgba;
    for (x, y, pixel) in out.enumerate_pixels_mut() {
        let nx = x as f32 / w as f32;
        let ny = y as f32 / h as f32;
        let dx = (nx - ecx) / erx;
        let dy = (ny - ecy) / ery;
        let inside = dx * dx + dy * dy <= 1.0;
        let dist = dist2([pixel[0], pixel[1], pixel[2]], bg);
        let border = nx < 0.08 || ny < 0.08 || nx > 0.92 || ny > 0.92;
        if !inside && (dist < 5_500 || (border && dist < 12_000)) {
            pixel[3] = 0;
        } else if !inside && dist < 9_000 {
            pixel[3] = 90;
        }
    }
    encode_png(&out)
}

/// 提高饱和度再色阶量化。强度只改档位，不生成珠板网格。
pub fn mock_cartoon(bytes: &[u8], strength: CartoonStrength) -> Result<Vec<u8>, VisionError> {
    let (levels, sat) = match strength {
        CartoonStrength::Light => (8u8, 1.12_f32),
        CartoonStrength::Standard => (5, 1.38),
        CartoonStrength::Strong => (3, 1.70),
    };
    let img = image::load_from_memory(bytes).map_err(|_| VisionError::Decode)?;
    let mut rgba = img.to_rgba8();
    for pixel in rgba.pixels_mut() {
        if pixel[3] == 0 {
            continue;
        }
        let rgb = posterize(saturate([pixel[0], pixel[1], pixel[2]], sat), levels);
        pixel[0] = rgb[0];
        pixel[1] = rgb[1];
        pixel[2] = rgb[2];
    }
    encode_png(&rgba)
}

pub fn strength_label(raw: &str) -> &'static str {
    CartoonStrength::parse(raw)
        .map(CartoonStrength::label)
        .unwrap_or("标准")
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

pub fn pipeline_tags(
    pipeline: &str,
    remove_bg: bool,
    strength: &str,
    subject: &str,
) -> Vec<String> {
    if pipeline != "mvp_plus" {
        return Vec::new();
    }
    let mut tags = Vec::new();
    if remove_bg {
        tags.push("智能去背".into());
    }
    tags.push(format!("卡通 · {}", strength_label(strength)));
    tags.push("拼豆量化".into());
    tags.push(subject_label(subject).into());
    tags
}

pub fn pipeline_lines(
    pipeline: &str,
    provider: &str,
    remove_bg: bool,
    strength: &str,
    pref: &str,
    brand: &str,
) -> Vec<String> {
    if pipeline != "mvp_plus" {
        return Vec::new();
    }
    let matte = if !remove_bg {
        "未去背".to_string()
    } else if provider == "aliyun" {
        "阿里抠图 · 已去背".to_string()
    } else {
        "开发模拟抠图 · 已去背".to_string()
    };
    let mut lines = vec![
        matte,
        format!("人物动漫化 · 强度{}", strength_label(strength)),
        format!("自研像素 / 色板 · {}", pref_label(pref)),
    ];
    if brand != "generic" && !brand.is_empty() {
        lines.push(format!("色盘品牌 · {}", brand_label(brand)));
    }
    lines
}

async fn aliyun_image(
    http: &reqwest::Client,
    host: &str,
    version: &str,
    action: &str,
    region: &str,
    key_id: &str,
    key_secret: &str,
    extra: &[(&str, &str)],
    label: &str,
) -> Result<Vec<u8>, VisionError> {
    let mut params: Vec<(String, String)> = vec![
        ("Format".into(), "JSON".into()),
        ("Version".into(), version.into()),
        ("AccessKeyId".into(), key_id.into()),
        ("SignatureMethod".into(), "HMAC-SHA1".into()),
        ("Timestamp".into(), utc_timestamp()),
        ("SignatureVersion".into(), "1.0".into()),
        ("SignatureNonce".into(), uuid::Uuid::new_v4().to_string()),
        ("Action".into(), action.into()),
        ("RegionId".into(), region.into()),
    ];
    for (k, v) in extra {
        params.push(((*k).to_string(), (*v).to_string()));
    }
    let signed = sign_rpc("POST", key_secret, &params);
    let url = format!("https://{host}/");
    let response = http
        .post(url)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(signed.body)
        .send()
        .await
        .map_err(|err| map_http(err, label))?;
    let status = response.status();
    let text = response.text().await.map_err(|err| map_http(err, label))?;
    let value: Value =
        serde_json::from_str(&text).unwrap_or(serde_json::json!({ "Message": text }));
    if !status.is_success() {
        return Err(remote_from(&value, label));
    }
    let Some(image_url) = extract_image_url(&value) else {
        return Err(remote_from(&value, label));
    };
    let downloaded = http
        .get(image_url)
        .send()
        .await
        .map_err(|err| map_http(err, label))?;
    if !downloaded.status().is_success() {
        return Err(VisionError::Remote(format!(
            "{label}结果下载失败。本次未扣积分。"
        )));
    }
    let bytes = downloaded
        .bytes()
        .await
        .map_err(|err| map_http(err, label))?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(VisionError::Remote(format!(
            "{label}结果过大。本次未扣积分。"
        )));
    }
    image::load_from_memory(&bytes).map_err(|_| VisionError::Decode)?;
    Ok(bytes.to_vec())
}

fn map_http(err: reqwest::Error, label: &str) -> VisionError {
    if err.is_timeout() {
        VisionError::Timeout
    } else {
        tracing::warn!(error = %err, label, "视觉请求失败");
        VisionError::Remote(format!("{label}失败，请稍后重试"))
    }
}

fn remote_from(value: &Value, label: &str) -> VisionError {
    let msg = value
        .get("Message")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let code = value
        .get("Code")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let text = if msg.is_empty() { code } else { msg };
    let short: String = text.chars().take(80).collect();
    if short.is_empty() {
        VisionError::Remote(format!("{label}失败，请稍后重试"))
    } else {
        tracing::warn!(code, message = %short, label, "阿里云返回错误");
        VisionError::Remote(format!("{label}失败：{short}"))
    }
}

fn extract_image_url(value: &Value) -> Option<String> {
    for pointer in ["/Data/ImageURL", "/ImageURL", "/Data/Url"] {
        if let Some(url) = value.pointer(pointer).and_then(|item| item.as_str()) {
            if url.starts_with("http://") || url.starts_with("https://") {
                return Some(url.to_string());
            }
        }
    }
    None
}

fn utc_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

struct SignedRpc {
    body: String,
}

/// 阿里云 RPC HMAC-SHA1。参数名按字典序，`*` 编成 `%2A`，空格编成 `%20`，`~` 不编码。
fn sign_rpc(method: &str, secret: &str, params: &[(String, String)]) -> SignedRpc {
    let mut pairs: Vec<(String, String)> = params
        .iter()
        .map(|(key, value)| (percent_encode(key), percent_encode(value)))
        .collect();
    pairs.sort();
    let canonical = pairs
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    let string_to_sign = format!(
        "{method}&{}&{}",
        percent_encode("/"),
        percent_encode(&canonical)
    );
    let key = format!("{secret}&");
    let mut mac = HmacSha1::new_from_slice(key.as_bytes()).expect("hmac key");
    mac.update(string_to_sign.as_bytes());
    let signature = STANDARD.encode(mac.finalize().into_bytes());
    let body = format!("{canonical}&Signature={}", percent_encode(&signature));
    SignedRpc { body }
}

pub fn percent_encode(input: &str) -> String {
    let mut out = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn corner_bg(img: &RgbaImage) -> [u8; 3] {
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut acc = [0u32; 3];
    let mut n = 0u32;
    for (cx, cy) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
        let pixel = img.get_pixel(cx.max(0) as u32, cy.max(0) as u32);
        acc[0] += pixel[0] as u32;
        acc[1] += pixel[1] as u32;
        acc[2] += pixel[2] as u32;
        n += 1;
    }
    [(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8]
}

fn dist2(a: [u8; 3], b: [u8; 3]) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
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

fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, VisionError> {
    let mut buf = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|_| VisionError::Decode)?;
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
    fn percent_encode_matches_aliyun_rules() {
        assert_eq!(percent_encode("a b*"), "a%20b%2A");
        assert_eq!(percent_encode("~"), "~");
        assert_eq!(
            percent_encode("2026-01-01T00:00:00Z"),
            "2026-01-01T00%3A00%3A00Z"
        );
    }

    #[test]
    fn signature_matches_independent_fixture() {
        let params = vec![
            ("Action".into(), "SegmentBody".into()),
            ("Format".into(), "JSON".into()),
            ("AccessKeyId".into(), "testid".into()),
            ("ImageURL".into(), "https://example.com/a.png".into()),
            ("RegionId".into(), "cn-shanghai".into()),
            ("SignatureMethod".into(), "HMAC-SHA1".into()),
            ("SignatureNonce".into(), "nonce-1".into()),
            ("SignatureVersion".into(), "1.0".into()),
            ("Timestamp".into(), "2026-01-01T00:00:00Z".into()),
            ("Version".into(), "2019-12-30".into()),
        ];
        let signed = sign_rpc("POST", "secret", &params);
        assert!(signed
            .body
            .contains("Signature=S16NMLcCyzB5R5F4E6M9qKbZ9dk%3D"));
        assert!(signed
            .body
            .starts_with("AccessKeyId=testid&Action=SegmentBody"));
    }

    #[test]
    fn mock_matte_clears_corner_and_keeps_center() {
        let src = png(48, 48, |x, y| {
            let dx = x as i32 - 24;
            let dy = y as i32 - 24;
            if dx * dx + dy * dy < 12 * 12 {
                Rgba([0xFF, 0x6B, 0x5B, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let matted = mock_matte(&src, SubjectKind::Person).unwrap();
        assert!(matted.starts_with(b"\x89PNG"));
        let img = image::load_from_memory(&matted).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0)[3], 0);
        assert_eq!(img.get_pixel(24, 24)[3], 255);
    }

    #[test]
    fn mock_cartoon_is_deterministic_and_not_a_grid() {
        let src = png(16, 16, |x, _| Rgba([80 + x as u8 * 8, 40, 90, 255]));
        let light = mock_cartoon(&src, CartoonStrength::Light).unwrap();
        let strong = mock_cartoon(&src, CartoonStrength::Strong).unwrap();
        let again = mock_cartoon(&src, CartoonStrength::Strong).unwrap();
        assert_eq!(strong, again);
        assert_ne!(light, strong);
        let img = image::load_from_memory(&strong).unwrap();
        assert_eq!((img.width(), img.height()), (16, 16));
    }

    #[test]
    fn transparent_pixel_survives_cartoon() {
        let src = png(8, 8, |x, _| {
            if x == 0 {
                Rgba([255, 0, 0, 0])
            } else {
                Rgba([10, 20, 30, 255])
            }
        });
        let out = mock_cartoon(&src, CartoonStrength::Standard).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0)[3], 0);
        assert_eq!(img.get_pixel(3, 3)[3], 255);
    }
}
