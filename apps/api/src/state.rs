use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::services::storage::LocalStorage;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
    pub storage: Arc<LocalStorage>,
}
