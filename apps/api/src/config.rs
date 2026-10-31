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
        }
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
