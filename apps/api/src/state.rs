use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::services::storage::LocalStorage;
use crate::services::wechat_pay::PlatformCertStore;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
    pub storage: Arc<LocalStorage>,
    pub pay_certs: PlatformCertStore,
}
