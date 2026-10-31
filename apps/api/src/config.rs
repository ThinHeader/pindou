//! 运行时配置（从环境变量 / .env 加载）

use std::env;

#[derive(Debug, Clone)]
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
        }
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn wechat_configured(&self) -> bool {
        !self.wx_appid.trim().is_empty() && !self.wx_secret.trim().is_empty()
    }
}
