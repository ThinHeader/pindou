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
    /// 火山方舟 API Key。留空且 `PIPELINE_PROVIDER=auto` 时走开发模拟。
    /// 不是阿里云 AccessKey，也不是视觉智能密钥。
    pub ark_api_key: String,
    /// 方舟 OpenAPI 根地址，默认 https://ark.cn-beijing.volces.com。
    pub ark_base_url: String,
    /// 默认 `doubao-seedream-5-0-flash-260915`。
    pub ark_model: String,
    /// `auto`（有方舟密钥走 Seedream，否则模拟）/ `mock` / `seedream` / `ark`。
    pub pipeline_provider: String,
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
            .field("pipeline_provider", &self.pipeline_provider)
            .field("ark_api_key", &redact_set(&self.ark_api_key))
            .field("ark_base_url", &self.ark_base_url())
            .field("ark_model", &self.ark_model())
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
            ark_api_key: env_trimmed("ARK_API_KEY"),
            ark_base_url: env_trimmed("ARK_BASE_URL"),
            ark_model: env_trimmed("ARK_MODEL"),
            pipeline_provider: env::var("PIPELINE_PROVIDER").unwrap_or_else(|_| "auto".into()),
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

    pub fn ark_ready(&self) -> bool {
        !self.ark_api_key.trim().is_empty()
    }

    pub fn ark_model(&self) -> &str {
        let model = self.ark_model.trim();
        if model.is_empty() {
            crate::services::seedream::SEEDREAM_MODEL_ID
        } else {
            model
        }
    }

    pub fn ark_base_url(&self) -> &str {
        let base = self.ark_base_url.trim().trim_end_matches('/');
        if base.is_empty() {
            crate::services::seedream::ARK_DEFAULT_BASE
        } else {
            base
        }
    }

    pub fn pipeline_mode(&self) -> Result<PipelineMode, &'static str> {
        resolve_provider(self.pipeline_provider.trim(), self.ark_api_key.trim())
    }

    /// 显式 mock，或 auto 且没有方舟密钥。不会请求火山方舟。
    pub fn use_mock_pipeline(&self) -> bool {
        matches!(self.pipeline_mode(), Ok(PipelineMode::Mock))
    }

    /// 写入任务的 provider。`seedream` 与 `ark` 都记成 `seedream`。
    pub fn provider_label(&self) -> Result<&'static str, &'static str> {
        match self.pipeline_mode()? {
            PipelineMode::Mock => Ok("mock"),
            PipelineMode::Seedream => Ok("seedream"),
        }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineMode {
    Mock,
    Seedream,
}

/// `auto`：有 `ARK_API_KEY` 走 Seedream，否则模拟。
/// `seedream` / `ark`：必须有方舟密钥，否则报错，不建任务。
/// `aliyun`：已退出默认路径，明确拒绝，避免误用视觉智能密钥。
pub fn resolve_provider(provider: &str, ark_api_key: &str) -> Result<PipelineMode, &'static str> {
    let key_ready = !ark_api_key.trim().is_empty();
    match provider.trim() {
        "mock" => Ok(PipelineMode::Mock),
        "seedream" | "ark" => {
            if key_ready {
                Ok(PipelineMode::Seedream)
            } else {
                Err("已指定火山方舟出稿，但未配置 ARK_API_KEY。不要使用阿里云视觉智能的 AccessKey。")
            }
        }
        "aliyun" => Err(
            "阿里云视觉智能（SegmentBody / GenerateHumanAnimeStyle）已不在出稿路径。请改用 PIPELINE_PROVIDER=seedream 或 ark，并配置火山方舟 ARK_API_KEY。",
        ),
        _ => {
            if key_ready {
                Ok(PipelineMode::Seedream)
            } else {
                Ok(PipelineMode::Mock)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_switch_follows_ark_key_and_rejects_aliyun() {
        assert_eq!(resolve_provider("auto", "").unwrap(), PipelineMode::Mock);
        assert_eq!(
            resolve_provider("auto", "ark-key").unwrap(),
            PipelineMode::Seedream
        );
        assert_eq!(
            resolve_provider("mock", "ark-key").unwrap(),
            PipelineMode::Mock
        );
        assert_eq!(
            resolve_provider("seedream", "ark-key").unwrap(),
            PipelineMode::Seedream
        );
        assert_eq!(
            resolve_provider("ark", " ark-key ").unwrap(),
            PipelineMode::Seedream
        );
        assert!(resolve_provider("seedream", "  ").is_err());
        assert!(resolve_provider("ark", "")
            .unwrap_err()
            .contains("ARK_API_KEY"));
        let aliyun = resolve_provider("aliyun", "LTAI-not-ark").unwrap_err();
        assert!(aliyun.contains("视觉智能"));
        assert!(aliyun.contains("ARK_API_KEY"));
    }
}
