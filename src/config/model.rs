use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::error::AppError;

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_admin_password() -> String {
    "change-me-in-production".to_string()
}

fn default_session_ttl_hours() -> u64 {
    72
}

fn default_database_url() -> String {
    "sqlite:///data/r2drive.db?mode=rwc".to_string()
}

fn default_sync_ttl_seconds() -> u64 {
    60
}

fn default_profile_name() -> String {
    "primary".to_string()
}

/// Server network and authentication configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_admin_password")]
    pub admin_password: String,
    #[serde(default)]
    pub jwt_secret: Option<String>,
    #[serde(default = "default_session_ttl_hours")]
    pub session_ttl_hours: u64,
    #[serde(default)]
    pub headless: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            admin_password: default_admin_password(),
            jwt_secret: None,
            session_ttl_hours: default_session_ttl_hours(),
            headless: false,
        }
    }
}

/// Database connection configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_database_url")]
    pub url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: default_database_url(),
        }
    }
}

/// Object sync and cache configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncConfig {
    #[serde(default = "default_sync_ttl_seconds")]
    pub default_ttl_seconds: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            default_ttl_seconds: default_sync_ttl_seconds(),
        }
    }
}

/// Credentials and settings for a Cloudflare R2 bucket profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BucketProfile {
    pub account_id: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket_name: String,
    #[serde(default)]
    pub public_url: Option<String>,
}

/// Root configuration structure for r2drive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub sync: SyncConfig,
    #[serde(default = "default_profile_name")]
    pub default_profile: String,
    #[serde(default)]
    pub profiles: HashMap<String, BucketProfile>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            sync: SyncConfig::default(),
            default_profile: default_profile_name(),
            profiles: HashMap::new(),
        }
    }
}

impl FromStr for Config {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::config::parse_config_str(s)
    }
}

impl Config {
    /// Load configuration from an optional explicit path or default discovery paths.
    pub fn load(path: Option<&Path>) -> Result<Self, AppError> {
        crate::config::load(path)
    }

    /// Parse configuration from a raw YAML string with environment substitution.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(content: &str) -> Result<Self, AppError> {
        content.parse()
    }

    /// Retrieve a bucket profile by name.
    pub fn get_profile(&self, name: &str) -> Option<&BucketProfile> {
        self.profiles.get(name)
    }

    /// Retrieve the default bucket profile.
    pub fn default_profile(&self) -> Option<&BucketProfile> {
        self.profiles.get(&self.default_profile)
    }
}
