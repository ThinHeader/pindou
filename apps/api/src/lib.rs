//! 拼豆 API。
//!
//! 启动时打开本地 SQLite（默认 `./data/pindou.db`）并执行 SQL migration。
//! 不再写入官方预置稿。

mod config;
mod error;
mod extract;
mod routes;
mod services;
mod state;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::extract::DefaultBodyLimit;
use axum::Router;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub use config::Config;
#[doc(hidden)]
pub use services::auth::issue_token;
pub use services::credits::china_today;
pub use services::session_seal::seal_session_key_for_tests;
pub use services::wechat_pay::seal_aead_for_tests;
pub use state::AppState;

/// 打开 SQLite 并执行 migration。
///
/// 单机单文件：WAL 让读写不互相阻塞，`busy_timeout` 兜住偶发争用，
/// 写事务由各业务显式 `BEGIN IMMEDIATE` 串行化（取代 Postgres 的 `FOR UPDATE`）。
pub async fn connect(config: &Config) -> anyhow::Result<AppState> {
    let sqlite = SqliteConnectOptions::from_str(&config.database_url)
        .with_context(|| format!("SQLite 连接串无效（{}）", config.database_url))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(10))
        .foreign_keys(true);

    let db_path = sqlite.get_filename().display().to_string();
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建 SQLite 目录失败（{}）", parent.display()))?;
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(sqlite)
        .await
        .with_context(|| format!("打开 SQLite 失败（{}）", redact_db(&config.database_url)))?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("执行 SQL migration 失败")?;


    let storage = Arc::new(
        services::storage::LocalStorage::new(&config.storage_dir, &config.public_base_url)
            .context("初始化本地对象存储失败")?,
    );

    let state = AppState {
        config: Arc::new(config.clone()),
        db: pool,
        storage,
        pay_certs: services::wechat_pay::PlatformCertStore::default(),
        wx_tokens: services::virtual_pay::AccessTokenStore::default(),
    };
    tracing::info!("{}", services::credits::describe_checkout(config));

    services::seed::seed_if_empty(&state).await?;

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

/// 本地 SQLite 没有账号密码，连接串本身可以直接进日志；只做脱敏兜底。
fn redact_db(url: &str) -> String {
    url.to_string()
}
