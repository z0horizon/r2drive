use std::collections::HashMap;
use crate::config::{BucketProfile, Config};
use crate::error::AppError;

/// Multi-bucket client pool managing `r2kit::R2Client` and `r2kit::Bucket` instances
/// for configured Cloudflare R2 bucket profiles.
#[derive(Clone, Debug)]
pub struct R2Manager {
    profiles: HashMap<String, BucketProfile>,
    clients: HashMap<String, r2kit::R2Client>,
    buckets: HashMap<String, r2kit::Bucket>,
    default_profile: Option<String>,
}

impl R2Manager {
    /// Create a new `R2Manager` from a `Config`.
    pub fn new(config: &Config) -> Result<Self, AppError> {
        Self::from_config(config)
    }

    /// Create an `R2Manager` by inspecting all profiles configured in `Config`.
    pub fn from_config(config: &Config) -> Result<Self, AppError> {
        let default_profile = if config.profiles.contains_key(&config.default_profile) {
            Some(config.default_profile.clone())
        } else if config.profiles.len() == 1 {
            config.profiles.keys().next().cloned()
        } else {
            None
        };

        Self::from_profiles(&config.profiles, default_profile)
    }

    /// Create an `R2Manager` from an explicit map of profiles and optional default profile name.
    pub fn from_profiles(
        profiles: &HashMap<String, BucketProfile>,
        default_profile: Option<String>,
    ) -> Result<Self, AppError> {
        let mut clients = HashMap::with_capacity(profiles.len());
        let mut buckets = HashMap::with_capacity(profiles.len());

        for (name, profile) in profiles {
            let r2_config = r2kit::R2Config::builder()
                .account_id(&profile.account_id)
                .access_key_id(&profile.access_key_id)
                .secret_access_key(&profile.secret_access_key)
                .build()
                .map_err(|e| {
                    AppError::Config(format!(
                        "Failed to build R2 configuration for profile '{name}': {e}"
                    ))
                })?;

            let client = r2kit::R2Client::new(r2_config);
            let bucket = client.bucket(&profile.bucket_name).map_err(|e| {
                AppError::Config(format!(
                    "Failed to resolve bucket '{}' for profile '{name}': {e}",
                    profile.bucket_name
                ))
            })?;

            clients.insert(name.clone(), client);
            buckets.insert(name.clone(), bucket);
        }

        Ok(Self {
            profiles: profiles.clone(),
            clients,
            buckets,
            default_profile,
        })
    }

    /// Get a handle to the `r2kit::Bucket` for a named profile.
    pub fn get_bucket(&self, profile_name: &str) -> Result<r2kit::Bucket, AppError> {
        self.buckets
            .get(profile_name)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Bucket profile '{profile_name}' not found")))
    }

    /// Get a handle to the default `r2kit::Bucket`, if configured.
    pub fn get_default_bucket(&self) -> Result<r2kit::Bucket, AppError> {
        let default_name = self
            .default_profile
            .as_deref()
            .ok_or_else(|| AppError::NotFound("No default profile configured".to_string()))?;
        self.get_bucket(default_name)
    }

    /// Get a handle to the `r2kit::R2Client` for a named profile.
    pub fn get_client(&self, profile_name: &str) -> Result<r2kit::R2Client, AppError> {
        self.clients
            .get(profile_name)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Bucket profile '{profile_name}' not found")))
    }

    /// Get the `BucketProfile` configuration for a named profile.
    pub fn get_profile(&self, profile_name: &str) -> Result<&BucketProfile, AppError> {
        self.profiles
            .get(profile_name)
            .ok_or_else(|| AppError::NotFound(format!("Bucket profile '{profile_name}' not found")))
    }

    /// Get the configured public URL (custom domain) for a named profile, if set.
    pub fn get_public_url(&self, profile_name: &str) -> Result<Option<&str>, AppError> {
        let profile = self.get_profile(profile_name)?;
        Ok(profile.public_url.as_deref())
    }

    /// Returns the default profile name, if configured.
    pub fn default_profile_name(&self) -> Option<&str> {
        self.default_profile.as_deref()
    }

    /// Returns all configured profile names.
    pub fn profile_names(&self) -> Vec<String> {
        self.profiles.keys().cloned().collect()
    }

    /// Returns reference to all profiles.
    pub fn profiles(&self) -> &HashMap<String, BucketProfile> {
        &self.profiles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_profile(bucket: &str) -> BucketProfile {
        BucketProfile {
            account_id: "0123456789abcdef0123456789abcdef".to_string(),
            access_key_id: "test-access-key".to_string(),
            secret_access_key: "test-secret-key".to_string(),
            bucket_name: bucket.to_string(),
            public_url: Some(format!("https://pub.{bucket}.com")),
        }
    }

    #[test]
    fn test_r2_manager_initialization_and_lookup() {
        let mut profiles = HashMap::new();
        profiles.insert("primary".to_string(), valid_profile("primary-bucket"));
        profiles.insert("secondary".to_string(), valid_profile("secondary-bucket"));

        let config = Config {
            default_profile: "primary".to_string(),
            profiles,
            ..Default::default()
        };

        let manager = R2Manager::new(&config).expect("Failed to initialize R2Manager");

        assert_eq!(manager.default_profile_name(), Some("primary"));
        assert_eq!(manager.profile_names().len(), 2);

        // Valid bucket lookup
        let bucket = manager.get_bucket("primary").unwrap();
        assert_eq!(bucket.name(), "primary-bucket");

        let default_bucket = manager.get_default_bucket().unwrap();
        assert_eq!(default_bucket.name(), "primary-bucket");

        let sec_bucket = manager.get_bucket("secondary").unwrap();
        assert_eq!(sec_bucket.name(), "secondary-bucket");

        // Profile lookup
        let prof = manager.get_profile("primary").unwrap();
        assert_eq!(prof.bucket_name, "primary-bucket");

        // Public URL lookup
        let pub_url = manager.get_public_url("primary").unwrap();
        assert_eq!(pub_url, Some("https://pub.primary-bucket.com"));

        // Client lookup
        assert!(manager.get_client("primary").is_ok());

        // Unknown profile lookup
        let not_found = manager.get_bucket("unknown");
        assert!(matches!(not_found, Err(AppError::NotFound(_))));
    }

    #[test]
    fn test_r2_manager_invalid_account_id_error() {
        let mut profiles = HashMap::new();
        let mut prof = valid_profile("my-bucket");
        prof.account_id = "short-acc".to_string(); // not 32 chars
        profiles.insert("invalid".to_string(), prof);

        let config = Config {
            profiles,
            ..Default::default()
        };

        let result = R2Manager::new(&config);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::Config(_)));
    }

    #[test]
    fn test_r2_manager_invalid_bucket_name_error() {
        let mut profiles = HashMap::new();
        let mut prof = valid_profile("my-bucket");
        prof.bucket_name = "ab".to_string(); // bucket name must be at least 3 chars
        profiles.insert("invalid".to_string(), prof);

        let config = Config {
            profiles,
            ..Default::default()
        };

        let result = R2Manager::new(&config);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::Config(_)));
    }

    #[test]
    fn test_r2_manager_missing_default_profile() {
        let profiles = HashMap::new();
        let config = Config {
            default_profile: "non-existent".to_string(),
            profiles,
            ..Default::default()
        };

        let manager = R2Manager::new(&config).unwrap();
        assert!(matches!(
            manager.get_default_bucket(),
            Err(AppError::NotFound(_))
        ));
    }
}
