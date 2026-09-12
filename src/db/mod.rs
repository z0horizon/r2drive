pub mod models;
pub mod repo;
pub mod sqlite;

use crate::error::AppError;
pub use repo::MetadataRepo;
pub use sqlite::SqliteMetadataRepo;
use std::sync::Arc;

pub async fn create_metadata_store(url: &str) -> Result<Arc<dyn MetadataRepo>, AppError> {
    if url.starts_with("sqlite:") {
        let repo = SqliteMetadataRepo::connect(url).await?;
        Ok(Arc::new(repo))
    } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
        Err(AppError::Config(
            "PostgreSQL backend is planned for Phase 2".to_string(),
        ))
    } else {
        Err(AppError::Config(format!(
            "Unsupported database URL scheme in: {url}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_metadata_store_sqlite() {
        let store = create_metadata_store("sqlite::memory:").await;
        assert!(store.is_ok());
    }

    #[tokio::test]
    async fn test_create_metadata_store_postgres_deferred() {
        let store = create_metadata_store("postgres://user:pass@localhost:5432/db").await;
        assert!(matches!(store, Err(AppError::Config(_))));
    }

    #[tokio::test]
    async fn test_create_metadata_store_unsupported() {
        let store = create_metadata_store("mysql://user:pass@localhost:3306/db").await;
        assert!(matches!(store, Err(AppError::Config(_))));
    }
}
