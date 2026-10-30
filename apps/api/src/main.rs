//! 拼豆 API 入口

mod config;
mod error;
mod routes;
mod services;

use std::sync::Arc;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;

/// 共享应用状态（后续注入 PgPool / Redis）
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    /// 内存占位：无 DB 时用的演示积分余额
    pub demo_credits: Arc<tokio::sync::Mutex<i32>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pindou_api=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let addr = config.bind_addr();

    let state = AppState {
        config: Arc::new(config),
        // 演示：今日剩余 2（日送 3 中已用 1）
        demo_credits: Arc::new(tokio::sync::Mutex::new(2)),
    };

    // 说明：sqlx PgPool / redis 连接按需在有 DATABASE_URL 可用时再建立。
    // 当前 stub 路由不强制连库，保证无 Docker 也能本地启动与 cargo check。
    // migrate 脚本后续放 migrations/，运行时 sqlx::migrate! 或 sqlx-cli。

    let app = Router::new()
        .merge(routes::router())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!("拼豆 API listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
