//! 运行时配置（从环境变量 / .env 加载）

use std::env;

#[derive(Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub wx_appid: String,
    pub wx_secret: String,
    /// 本地对象存储目录（COS 的占位实现）
    pub storage_dir: String,
    /// 对外访问文件的基址，如 http://127.0.0.1:3000
    pub public_base_url: String,
    /// 阿里云 AccessKey。留空则 MVP+ 走开发模拟（不请求 VIAPI）。
    pub aliyun_access_key_id: String,
    pub aliyun_access_key_secret: String,
    /// 视觉智能区域，默认 cn-shanghai。
    pub aliyun_region: String,
    /// `auto`（有密钥走阿里云，否则模拟）/ `mock` / `aliyun`。
    pub pipeline_provider: String,
    /// 可选，覆盖 imageseg 主机名，不含协议。
    pub aliyun_imageseg_endpoint: String,
    /// 可选，覆盖 imageenhan 主机名，不含协议。
    pub aliyun_imageenhan_endpoint: String,
    /// 小程序 AppID。留空时回落到 `WX_APPID`（登录和支付必须是同一个小程序）。
    pub wechat_app_id: String,
    pub wechat_mch_id: String,
    pub wechat_mch_serial_no: String,
    /// 商户 API 私钥 PEM。来自环境变量内容或文件。
    pub wechat_mch_private_key: String,
    /// 微信支付 APIv3 密钥，32 位。
    pub wechat_pay_api_v3_key: String,
    /// 支付结果通知 URL。留空时用 `PUBLIC_BASE_URL/api/v1/pay/wechat/notify`。
    pub wechat_pay_notify_url: String,
    /// 微信支付公钥 PEM，或平台证书 PEM。与 `wechat_pay_public_key_id` 成对出现。
    pub wechat_pay_public_key: String,
    pub wechat_pay_public_key_id: String,
    /// 默认 `https://api.mch.weixin.qq.com`。测试可改到本机假微信。
    pub wechat_pay_api_base: String,
    /// 任一商户参数非空。此时禁止开发示意到账。
    pub pay_credentials_present: bool,
    /// 私钥文件读失败等加载错误。有值时支付不可用，示意到账同样关闭。
    pub pay_config_error: Option<String>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database_url", &"(redacted)")
            .field("jwt_secret", &"(redacted)")
            .field("wx_appid", &self.wx_appid)
            .field("wx_secret", &"(redacted)")
            .field("storage_dir", &self.storage_dir)
            .field("public_base_url", &self.public_base_url)
            .field("aliyun_region", &self.aliyun_region)
            .field("pipeline_provider", &self.pipeline_provider)
            .field(
                "aliyun_access_key_id",
                &if self.aliyun_access_key_id.trim().is_empty() {
                    "(empty)"
                } else {
                    "(set)"
                },
            )
            .field("aliyun_access_key_secret", &"(redacted)")
            .field("aliyun_imageseg_endpoint", &self.aliyun_imageseg_endpoint)
            .field(
                "aliyun_imageenhan_endpoint",
                &self.aliyun_imageenhan_endpoint,
            )
            .field("wechat_app_id", &self.mini_app_id())
            .field("wechat_mch_id", &self.wechat_mch_id)
            .field("wechat_mch_serial_no", &self.wechat_mch_serial_no)
            .field(
                "wechat_mch_private_key",
                &redact_set(&self.wechat_mch_private_key),
            )
            .field(
                "wechat_pay_api_v3_key",
                &redact_set(&self.wechat_pay_api_v3_key),
            )
            .field("wechat_pay_notify_url", &self.wechat_pay_notify_url)
            .field(
                "wechat_pay_public_key",
                &redact_set(&self.wechat_pay_public_key),
            )
            .field("wechat_pay_public_key_id", &self.wechat_pay_public_key_id)
            .field("wechat_pay_api_base", &self.wechat_pay_api_base)
            .field("pay_credentials_present", &self.pay_credentials_present)
            .field("pay_config_error", &self.pay_config_error)
            .finish()
    }
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();

        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);
        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());

        Self {
            host: host.clone(),
            port,
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://pindou:pindou_dev@127.0.0.1:5432/pindou".into()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "dev-only-jwt-secret-change-me".into()),
            wx_appid: env::var("WX_APPID").unwrap_or_default(),
            wx_secret: env::var("WX_SECRET").unwrap_or_default(),
            storage_dir: env::var("STORAGE_DIR").unwrap_or_else(|_| "./data/objects".into()),
            public_base_url: env::var("PUBLIC_BASE_URL")
                .unwrap_or_else(|_| format!("http://127.0.0.1:{port}")),
            aliyun_access_key_id: env::var("ALIYUN_ACCESS_KEY_ID").unwrap_or_default(),
            aliyun_access_key_secret: env::var("ALIYUN_ACCESS_KEY_SECRET").unwrap_or_default(),
            aliyun_region: env::var("ALIYUN_REGION").unwrap_or_else(|_| "cn-shanghai".into()),
            pipeline_provider: env::var("PIPELINE_PROVIDER").unwrap_or_else(|_| "auto".into()),
            aliyun_imageseg_endpoint: env::var("ALIYUN_IMAGESEG_ENDPOINT").unwrap_or_default(),
            aliyun_imageenhan_endpoint: env::var("ALIYUN_IMAGEENHAN_ENDPOINT").unwrap_or_default(),
            wechat_app_id: env_trimmed("WECHAT_APP_ID"),
            wechat_mch_id: env_trimmed("WECHAT_MCH_ID"),
            wechat_mch_serial_no: env_trimmed("WECHAT_MCH_SERIAL_NO"),
            wechat_mch_private_key: String::new(),
            wechat_pay_api_v3_key: env_trimmed("WECHAT_PAY_API_V3_KEY"),
            wechat_pay_notify_url: env_trimmed("WECHAT_PAY_NOTIFY_URL"),
            wechat_pay_public_key: String::new(),
            wechat_pay_public_key_id: env_trimmed("WECHAT_PAY_PUBLIC_KEY_ID"),
            wechat_pay_api_base: {
                let raw = env_trimmed("WECHAT_PAY_API_BASE");
                if raw.is_empty() {
                    "https://api.mch.weixin.qq.com".into()
                } else {
                    raw
                }
            },
            pay_credentials_present: false,
            pay_config_error: None,
        }
        .load_pay_material()
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn wechat_configured(&self) -> bool {
        !self.wx_appid.trim().is_empty() && !self.wx_secret.trim().is_empty()
    }

    pub fn aliyun_ready(&self) -> bool {
        !self.aliyun_access_key_id.trim().is_empty()
            && !self.aliyun_access_key_secret.trim().is_empty()
    }

    /// 密钥缺失或显式 `PIPELINE_PROVIDER=mock` 时不请求阿里云。
    pub fn use_mock_pipeline(&self) -> bool {
        match self.pipeline_provider.trim() {
            "mock" => true,
            "aliyun" => false,
            _ => !self.aliyun_ready(),
        }
    }

    pub fn aliyun_region_id(&self) -> &str {
        let region = self.aliyun_region.trim();
        if region.is_empty() {
            "cn-shanghai"
        } else {
            region
        }
    }

    pub fn imageseg_host(&self) -> String {
        host_or(
            &self.aliyun_imageseg_endpoint,
            &format!("imageseg.{}.aliyuncs.com", self.aliyun_region_id()),
        )
    }

    pub fn imageenhan_host(&self) -> String {
        host_or(
            &self.aliyun_imageenhan_endpoint,
            &format!("imageenhan.{}.aliyuncs.com", self.aliyun_region_id()),
        )
    }

    /// 支付使用的小程序 AppID。`WECHAT_APP_ID` 优先，否则用登录的 `WX_APPID`。
    pub fn mini_app_id(&self) -> &str {
        if !self.wechat_app_id.trim().is_empty() {
            self.wechat_app_id.trim()
        } else {
            self.wx_appid.trim()
        }
    }

    /// 集成测试用：丢掉商户参数，强制走开发示意充值，避免打到真微信。
    pub fn force_mock_pay(&mut self) {
        self.wechat_mch_id.clear();
        self.wechat_mch_serial_no.clear();
        self.wechat_mch_private_key.clear();
        self.wechat_pay_api_v3_key.clear();
        self.wechat_pay_notify_url.clear();
        self.wechat_pay_public_key.clear();
        self.wechat_pay_public_key_id.clear();
        self.pay_credentials_present = false;
        self.pay_config_error = None;
    }

    fn load_pay_material(mut self) -> Self {
        let private_key = load_pem("WECHAT_MCH_PRIVATE_KEY", "WECHAT_MCH_PRIVATE_KEY_PATH");
        let public_key = load_pem("WECHAT_PAY_PUBLIC_KEY", "WECHAT_PAY_PUBLIC_KEY_PATH");
        self.wechat_mch_private_key = private_key.pem;
        self.wechat_pay_public_key = public_key.pem;
        self.pay_config_error = private_key.error.or(public_key.error);
        let present = !self.wechat_mch_id.is_empty()
            || !self.wechat_mch_serial_no.is_empty()
            || private_key.attempted
            || !self.wechat_pay_api_v3_key.is_empty()
            || public_key.attempted
            || !self.wechat_pay_public_key_id.is_empty()
            || !self.wechat_pay_notify_url.is_empty();
        self.pay_credentials_present = present;
        self
    }
}

struct LoadedPem {
    attempted: bool,
    pem: String,
    error: Option<String>,
}

fn env_trimmed(key: &str) -> String {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
}

fn redact_set(value: &str) -> &'static str {
    if value.trim().is_empty() {
        "(empty)"
    } else {
        "(set)"
    }
}

fn load_pem(inline_key: &str, path_key: &str) -> LoadedPem {
    if let Some(path) = env::var(path_key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return match read_pem_file(&path) {
            Ok(pem) => LoadedPem {
                attempted: true,
                pem,
                error: None,
            },
            Err(err) => LoadedPem {
                attempted: true,
                pem: String::new(),
                error: Some(format!("{path_key} 无法读取：{err}")),
            },
        };
    }

    let Some(raw) = env::var(inline_key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return LoadedPem {
            attempted: false,
            pem: String::new(),
            error: None,
        };
    };

    let normalized = raw.replace("\\n", "\n");
    if normalized.contains("BEGIN") {
        return LoadedPem {
            attempted: true,
            pem: normalized,
            error: None,
        };
    }
    if std::path::Path::new(raw.trim()).is_file() {
        return match read_pem_file(raw.trim()) {
            Ok(pem) => LoadedPem {
                attempted: true,
                pem,
                error: None,
            },
            Err(err) => LoadedPem {
                attempted: true,
                pem: String::new(),
                error: Some(format!("{inline_key} 无法读取：{err}")),
            },
        };
    }
    LoadedPem {
        attempted: true,
        pem: String::new(),
        error: Some(format!("{inline_key} 不是 PEM，也不是可读文件路径")),
    }
}

fn read_pem_file(path: &str) -> std::io::Result<String> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > 64 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "文件过大",
        ));
    }
    Ok(std::fs::read_to_string(path)?.replace("\\n", "\n"))
}

fn host_or(raw: &str, fallback: &str) -> String {
    let trimmed = raw
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}
