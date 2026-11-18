use std::sync::Arc;

use sqlx::SqlitePool;

use crate::config::Config;
use crate::services::storage::LocalStorage;
use crate::services::virtual_pay::AccessTokenStore;
use crate::services::wechat_pay::PlatformCertStore;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: SqlitePool,
    pub storage: Arc<LocalStorage>,
    pub pay_certs: PlatformCertStore,
    pub wx_tokens: AccessTokenStore,
}
