pub mod cat;
pub mod download;
pub mod ls;
pub mod rm;
pub mod upload;

use crate::config::Config;
use crate::error::AppError;
use crate::r2::R2Manager;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

/// Command-line interface for r2drive.
#[derive(Parser, Debug)]
#[command(
    name = "r2drive",
    version,
    about = "Cloudflare R2 object storage drive and REST service",
    long_about = None
)]
pub struct Cli {
    /// Optional path to config.yaml
    #[arg(short = 'c', long = "config", global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Optional bucket profile name (falls back to config.default_profile or first profile)
    #[arg(short = 'p', long = "profile", global = true, value_name = "NAME")]
    pub profile: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List objects in the bucket
    Ls {
        /// Optional key prefix to filter objects
        #[arg(value_name = "PREFIX")]
        prefix: Option<String>,

        /// List recursively without delimiter grouping
        #[arg(short = 'r', long = "recursive")]
        recursive: bool,

        /// Output results as JSON
        #[arg(long = "json")]
        json: bool,
    },

    /// Upload a local file to R2
    Upload {
        /// Local file path to upload
        #[arg(value_name = "LOCAL_PATH")]
        local_path: PathBuf,

        /// Remote object key (defaults to local file name)
        #[arg(value_name = "REMOTE_KEY")]
        remote_key: Option<String>,
    },

    /// Download an object from R2 to a local file
    Download {
        /// Remote object key to download
        #[arg(value_name = "REMOTE_KEY")]
        remote_key: String,

        /// Destination local file path
        #[arg(value_name = "LOCAL_PATH")]
        local_path: PathBuf,
    },

    /// Stream an object's contents to stdout
    Cat {
        /// Remote object key to stream
        #[arg(value_name = "REMOTE_KEY")]
        remote_key: String,
    },

    /// Delete an object from R2
    Rm {
        /// Remote object key to delete
        #[arg(value_name = "REMOTE_KEY")]
        remote_key: String,
    },

    /// Start the r2drive REST API server and web UI
    Serve {
        /// Port to bind the server to (overrides config)
        #[arg(short = 'P', long = "port", value_name = "PORT")]
        port: Option<u16>,

        /// Run in headless mode without opening web UI
        #[arg(long = "headless")]
        headless: bool,
    },
}

/// Resolves a bucket and profile name from config and optional CLI profile override.
///
/// Precedence:
/// 1. Explicit profile specified via `--profile`
/// 2. Profile specified in `config.default_profile`
/// 3. First available profile in `config.profiles`
pub fn resolve_bucket_with_profile(
    config_path: Option<&Path>,
    profile_override: Option<&str>,
) -> Result<(String, r2kit::Bucket), AppError> {
    let config = Config::load(config_path)?;
    let manager = R2Manager::new(&config)?;

    if let Some(profile_name) = profile_override {
        let bucket = manager.get_bucket(profile_name)?;
        return Ok((profile_name.to_string(), bucket));
    }

    if let Some(default_profile) = manager.default_profile_name()
        && let Ok(bucket) = manager.get_bucket(default_profile)
    {
        return Ok((default_profile.to_string(), bucket));
    }

    let mut profile_names: Vec<_> = config.profiles.keys().cloned().collect();
    profile_names.sort();

    if let Some(first_profile) = profile_names.first() {
        let bucket = manager.get_bucket(first_profile)?;
        return Ok((first_profile.clone(), bucket));
    }

    Err(AppError::Config(
        "No bucket profiles configured in config.yaml".to_string(),
    ))
}

/// Resolves a bucket from config and optional CLI profile override.
pub fn resolve_bucket(
    config_path: Option<&Path>,
    profile_override: Option<&str>,
) -> Result<r2kit::Bucket, AppError> {
    resolve_bucket_with_profile(config_path, profile_override).map(|(_, bucket)| bucket)
}

impl Cli {
    /// Execute the parsed CLI command.
    pub async fn execute(&self) -> Result<(), AppError> {
        let config_path = self.config.as_deref();
        let profile = self.profile.as_deref();

        match &self.command {
            Commands::Ls {
                prefix,
                recursive,
                json,
            } => {
                let bucket = resolve_bucket(config_path, profile)?;
                ls::execute(&bucket, prefix.as_deref(), *recursive, *json).await?;
            }
            Commands::Upload {
                local_path,
                remote_key,
            } => {
                let bucket = resolve_bucket(config_path, profile)?;
                upload::execute(&bucket, local_path, remote_key.as_deref()).await?;
            }
            Commands::Download {
                remote_key,
                local_path,
            } => {
                let bucket = resolve_bucket(config_path, profile)?;
                download::execute(&bucket, remote_key, local_path).await?;
            }
            Commands::Cat { remote_key } => {
                let bucket = resolve_bucket(config_path, profile)?;
                cat::execute(&bucket, remote_key).await?;
            }
            Commands::Rm { remote_key } => {
                let bucket = resolve_bucket(config_path, profile)?;
                rm::execute(&bucket, remote_key).await?;
            }
            Commands::Serve { port, headless } => {
                println!(
                    "Server will be implemented in Task 6 (port: {:?}, headless: {})",
                    port, headless
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BucketProfile;
    use std::collections::HashMap;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_test_config(profiles: HashMap<String, BucketProfile>, default_profile: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        let config = Config {
            default_profile: default_profile.to_string(),
            profiles,
            ..Default::default()
        };
        let yaml = serde_yaml::to_string(&config).unwrap();
        file.write_all(yaml.as_bytes()).unwrap();
        file
    }

    fn sample_profile(bucket: &str) -> BucketProfile {
        BucketProfile {
            account_id: "0123456789abcdef0123456789abcdef".to_string(),
            access_key_id: "test-access-key".to_string(),
            secret_access_key: "test-secret-key".to_string(),
            bucket_name: bucket.to_string(),
            public_url: None,
        }
    }

    #[test]
    fn test_resolve_bucket_with_explicit_profile() {
        let mut profiles = HashMap::new();
        profiles.insert("primary".to_string(), sample_profile("bucket-pri"));
        profiles.insert("secondary".to_string(), sample_profile("bucket-sec"));

        let file = make_test_config(profiles, "primary");
        let (prof, bucket) = resolve_bucket_with_profile(Some(file.path()), Some("secondary")).unwrap();
        assert_eq!(prof, "secondary");
        assert_eq!(bucket.name(), "bucket-sec");
    }

    #[test]
    fn test_resolve_bucket_with_default_profile() {
        let mut profiles = HashMap::new();
        profiles.insert("primary".to_string(), sample_profile("bucket-pri"));
        profiles.insert("secondary".to_string(), sample_profile("bucket-sec"));

        let file = make_test_config(profiles, "primary");
        let (prof, bucket) = resolve_bucket_with_profile(Some(file.path()), None).unwrap();
        assert_eq!(prof, "primary");
        assert_eq!(bucket.name(), "bucket-pri");
    }

    #[test]
    fn test_resolve_bucket_fallback_to_first_profile() {
        let mut profiles = HashMap::new();
        profiles.insert("alpha".to_string(), sample_profile("bucket-alpha"));
        profiles.insert("beta".to_string(), sample_profile("bucket-beta"));

        // default_profile points to non-existent profile
        let file = make_test_config(profiles, "non-existent");
        let (prof, bucket) = resolve_bucket_with_profile(Some(file.path()), None).unwrap();
        assert_eq!(prof, "alpha");
        assert_eq!(bucket.name(), "bucket-alpha");
    }

    #[test]
    fn test_resolve_bucket_unknown_profile_error() {
        let mut profiles = HashMap::new();
        profiles.insert("primary".to_string(), sample_profile("bucket-pri"));

        let file = make_test_config(profiles, "primary");
        let result = resolve_bucket_with_profile(Some(file.path()), Some("unknown"));
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn test_resolve_bucket_no_profiles_error() {
        let file = make_test_config(HashMap::new(), "primary");
        let result = resolve_bucket_with_profile(Some(file.path()), None);
        assert!(matches!(result, Err(AppError::Config(_))));
    }

    #[test]
    fn test_cli_parser_subcommands() {
        let cli = Cli::try_parse_from(["r2drive", "ls", "prefix/sub", "-r", "--json"]).unwrap();
        match cli.command {
            Commands::Ls {
                prefix,
                recursive,
                json,
            } => {
                assert_eq!(prefix.as_deref(), Some("prefix/sub"));
                assert!(recursive);
                assert!(json);
            }
            _ => panic!("Expected Ls subcommand"),
        }

        let cli = Cli::try_parse_from(["r2drive", "upload", "local.txt", "remote.txt"]).unwrap();
        match cli.command {
            Commands::Upload {
                local_path,
                remote_key,
            } => {
                assert_eq!(local_path, PathBuf::from("local.txt"));
                assert_eq!(remote_key.as_deref(), Some("remote.txt"));
            }
            _ => panic!("Expected Upload subcommand"),
        }

        let cli = Cli::try_parse_from(["r2drive", "download", "remote.txt", "local.txt"]).unwrap();
        match cli.command {
            Commands::Download {
                remote_key,
                local_path,
            } => {
                assert_eq!(remote_key, "remote.txt");
                assert_eq!(local_path, PathBuf::from("local.txt"));
            }
            _ => panic!("Expected Download subcommand"),
        }

        let cli = Cli::try_parse_from(["r2drive", "cat", "file.bin"]).unwrap();
        match cli.command {
            Commands::Cat { remote_key } => {
                assert_eq!(remote_key, "file.bin");
            }
            _ => panic!("Expected Cat subcommand"),
        }

        let cli = Cli::try_parse_from(["r2drive", "rm", "file.bin"]).unwrap();
        match cli.command {
            Commands::Rm { remote_key } => {
                assert_eq!(remote_key, "file.bin");
            }
            _ => panic!("Expected Rm subcommand"),
        }

        let cli = Cli::try_parse_from(["r2drive", "serve", "-P", "9090", "--headless"]).unwrap();
        match cli.command {
            Commands::Serve { port, headless } => {
                assert_eq!(port, Some(9090));
                assert!(headless);
            }
            _ => panic!("Expected Serve subcommand"),
        }
    }
}
