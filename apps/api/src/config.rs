//! 运行时配置（从环境变量 / .env 加载）

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub redis_url: String,
    pub jwt_secret: String,
    pub wx_appid: String,
    pub wx_secret: String,
}

impl Config {
    pub fn from_env() -> Self {
        // .env 缺失时不 panic，方便无 DB 也能 cargo check / 本地 stub 跑
        let _ = dotenvy::dotenv();

        Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://pindou:pindou_dev@127.0.0.1:5432/pindou".into()
            }),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".into()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "dev-only-jwt-secret-change-me".into()),
            wx_appid: env::var("WX_APPID").unwrap_or_default(),
            wx_secret: env::var("WX_SECRET").unwrap_or_default(),
        }
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
