//! 拼豆 API。
//!
//! 启动时连接 PostgreSQL / Redis、执行 SQL migration，并写入官方模板种子数据。

mod config;
mod error;
mod extract;
mod routes;
mod services;
mod state;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::extract::DefaultBodyLimit;
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub use config::Config;
pub use services::credits::{china_today, daily_claim_key};
pub use state::AppState;

/// 连接依赖、迁移、种子数据。
pub async fn connect(config: &Config) -> anyhow::Result<AppState> {
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(8))
        .connect(&config.database_url)
        .await
        .with_context(|| {
            format!(
                "连接 PostgreSQL 失败（{}）",
                redact_db(&config.database_url)
            )
        })?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("执行 SQL migration 失败")?;

    let redis_client =
        redis::Client::open(config.redis_url.as_str()).with_context(|| "Redis URL 无效")?;
    let redis = redis::aio::ConnectionManager::new(redis_client)
        .await
        .context("连接 Redis 失败")?;

    let storage = Arc::new(
        services::storage::LocalStorage::new(&config.storage_dir, &config.public_base_url)
            .context("初始化本地对象存储失败")?,
    );

    let state = AppState {
        config: Arc::new(config.clone()),
        db: pool,
        redis,
        storage,
    };

    services::seed::seed_if_empty(&state)
        .await
        .context("写入官方模板种子失败")?;

    Ok(state)
}

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .merge(routes::health::router())
        .nest("/api/v1", routes::api_v1())
        .route("/files/{*path}", axum::routing::get(routes::files::serve))
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn serve(config: Config) -> anyhow::Result<()> {
    let addr = config.bind_addr();
    let state = connect(&config).await?;
    let app = router(state);
    tracing::info!("拼豆 API listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn redact_db(url: &str) -> String {
    // 避免日志带出密码：postgres://user:pass@host/db → postgres://user:***@host/db
    if let Some(scheme_end) = url.find("://") {
        let rest = &url[scheme_end + 3..];
        if let Some(at) = rest.find('@') {
            let auth = &rest[..at];
            if let Some(colon) = auth.find(':') {
                let user = &auth[..colon];
                return format!("{}{}:***@{}", &url[..scheme_end + 3], user, &rest[at + 1..]);
            }
        }
    }
    "postgres://***".into()
}
