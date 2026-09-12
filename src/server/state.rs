use crate::config::Config;
use crate::db::MetadataRepo;
use crate::r2::R2Manager;
use std::sync::Arc;

/// Shared application state across HTTP handlers and background tasks.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Arc<dyn MetadataRepo>,
    pub r2: Arc<R2Manager>,
}

impl AppState {
    pub fn new(config: Arc<Config>, db: Arc<dyn MetadataRepo>, r2: Arc<R2Manager>) -> Self {
        Self { config, db, r2 }
    }
}
