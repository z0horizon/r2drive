pub mod model;

use std::path::{Path, PathBuf};
use crate::error::AppError;

pub use model::{BucketProfile, Config, DatabaseConfig, ServerConfig, SyncConfig};

fn substitute_line(line: &str) -> Result<String, AppError> {
    let mut result = String::with_capacity(line.len());
    let mut chars = line.char_indices().peekable();
    let mut last_idx = 0;

    while let Some((i, c)) = chars.next() {
        if c != '$' {
            continue;
        }

        let Some(&(next_i, next_c)) = chars.peek() else {
            continue;
        };

        if next_c == '$' {
            let after_second_dollar = next_i + next_c.len_utf8();
            if line[after_second_dollar..].starts_with('{') {
                // Escaped placeholder: "$${VAR}" -> "${VAR}"
                result.push_str(&line[last_idx..i]);
                result.push('$');
                chars.next(); // consume second '$'
                last_idx = after_second_dollar;
            }
        } else if next_c == '{' {
            result.push_str(&line[last_idx..i]);
            chars.next(); // consume '{'
            let start_var = next_i + 1;
            let mut end_var = None;

            for (j, c2) in chars.by_ref() {
                if c2 == '}' {
                    end_var = Some(j);
                    break;
                }
            }

            let end = end_var.ok_or_else(|| {
                AppError::Config(
                    "Unclosed environment variable substitution: missing '}'".to_string(),
                )
            })?;

            let var_spec = &line[start_var..end];
            if var_spec.trim().is_empty() {
                return Err(AppError::Config(
                    "Empty environment variable substitution '${}'".to_string(),
                ));
            }

            let (var_name, default_val) = if let Some(sep) = var_spec.find(":-") {
                (&var_spec[..sep], Some(&var_spec[sep + 2..]))
            } else {
                (var_spec, None)
            };

            let val = match std::env::var(var_name) {
                Ok(v) if !v.is_empty() => v,
                Ok(_) => match default_val {
                    Some(def) => def.to_string(),
                    None => String::new(),
                },
                Err(_) => match default_val {
                    Some(def) => def.to_string(),
                    None => {
                        return Err(AppError::Config(format!(
                            "Environment variable '{}' is not set",
                            var_name
                        )));
                    }
                },
            };

            result.push_str(&val);
            last_idx = end + 1;
        }
    }
    result.push_str(&line[last_idx..]);
    Ok(result)
}

/// Substitute `${VAR_NAME}` and `${VAR_NAME:-default}` occurrences in raw configuration string
/// using environment variables. Commented lines starting with `#` are preserved as-is.
pub fn substitute_env_vars(raw: &str) -> Result<String, AppError> {
    let mut result = String::with_capacity(raw.len());
    for line in raw.split_inclusive('\n') {
        if line.trim_start().starts_with('#') {
            result.push_str(line);
        } else {
            result.push_str(&substitute_line(line)?);
        }
    }
    Ok(result)
}

/// Parse configuration from a YAML string, performing environment variable substitution.
pub fn parse_config_str(content: &str) -> Result<Config, AppError> {
    let substituted = substitute_env_vars(content)?;
    serde_yaml::from_str::<Config>(&substituted).map_err(|e| {
        AppError::Config(format!("Failed to parse YAML configuration: {e}"))
    })
}

/// Resolve default configuration file paths in order of precedence:
/// 1. `~/.config/r2drive/config.yaml`
/// 2. `/etc/r2drive/config.yaml`
/// 3. `./config.yaml`
pub fn resolve_default_config_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(home) = std::env::var("HOME").ok().filter(|h| !h.trim().is_empty()) {
        candidates.push(
            PathBuf::from(home)
                .join(".config")
                .join("r2drive")
                .join("config.yaml"),
        );
    }

    candidates.push(PathBuf::from("/etc/r2drive/config.yaml"));
    candidates.push(PathBuf::from("./config.yaml"));

    candidates.into_iter().find(|p| p.is_file())
}

/// Load configuration from a specific file path.
pub fn load_from_path(path: &Path) -> Result<Config, AppError> {
    if !path.exists() {
        return Err(AppError::Config(format!(
            "Configuration file not found: {}",
            path.display()
        )));
    }
    let content = std::fs::read_to_string(path).map_err(|e| {
        AppError::Config(format!(
            "Failed to read configuration file '{}': {}",
            path.display(),
            e
        ))
    })?;
    parse_config_str(&content)
}

/// Load configuration by checking explicit path, `$R2DRIVE_CONFIG`, or default search paths.
pub fn load(path: Option<&Path>) -> Result<Config, AppError> {
    if let Some(explicit_path) = path {
        return load_from_path(explicit_path);
    }

    if let Some(env_path) = std::env::var("R2DRIVE_CONFIG")
        .ok()
        .filter(|p| !p.trim().is_empty())
    {
        return load_from_path(Path::new(&env_path));
    }

    if let Some(default_path) = resolve_default_config_path() {
        return load_from_path(&default_path);
    }

    Err(AppError::Config(
        "No configuration file found. Checked explicit path, $R2DRIVE_CONFIG, ~/.config/r2drive/config.yaml, /etc/r2drive/config.yaml, and ./config.yaml".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    // Mutex to avoid environment variable races in parallel tests
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_parse_valid_yaml() {
        let yaml = r#"
server:
  host: "127.0.0.1"
  port: 9000
  admin_password: "secretpassword"
  session_ttl_hours: 48
  headless: true
database:
  url: "sqlite://test.db"
sync:
  default_ttl_seconds: 120
default_profile: "test"
profiles:
  test:
    account_id: "acc123"
    access_key_id: "key123"
    secret_access_key: "secret123"
    bucket_name: "my-bucket"
"#;
        let config: Config = parse_config_str(yaml).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.server.admin_password, "secretpassword");
        assert_eq!(config.server.session_ttl_hours, 48);
        assert!(config.server.headless);
        assert_eq!(config.database.url, "sqlite://test.db");
        assert_eq!(config.sync.default_ttl_seconds, 120);
        assert_eq!(config.default_profile, "test");
        assert!(config.profiles.contains_key("test"));
        let profile = config.get_profile("test").unwrap();
        assert_eq!(profile.account_id, "acc123");
        assert_eq!(profile.access_key_id, "key123");
        assert_eq!(profile.secret_access_key, "secret123");
        assert_eq!(profile.bucket_name, "my-bucket");
        assert_eq!(profile.public_url, None);
    }

    #[test]
    fn test_env_var_substitution() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::set_var("TEST_R2_ACC", "account_abc");
            std::env::set_var("TEST_R2_KEY", "key_def");
            std::env::set_var("TEST_R2_SECRET", "secret_ghi");
            std::env::set_var("TEST_R2_BUCKET", "bucket_jkl");
        }

        let yaml = r#"
profiles:
  primary:
    account_id: "${TEST_R2_ACC}"
    access_key_id: "${TEST_R2_KEY}"
    secret_access_key: "${TEST_R2_SECRET}"
    bucket_name: "${TEST_R2_BUCKET}"
"#;
        let config = parse_config_str(yaml).unwrap();
        let profile = config.get_profile("primary").unwrap();
        assert_eq!(profile.account_id, "account_abc");
        assert_eq!(profile.access_key_id, "key_def");
        assert_eq!(profile.secret_access_key, "secret_ghi");
        assert_eq!(profile.bucket_name, "bucket_jkl");

        unsafe {
            std::env::remove_var("TEST_R2_ACC");
            std::env::remove_var("TEST_R2_KEY");
            std::env::remove_var("TEST_R2_SECRET");
            std::env::remove_var("TEST_R2_BUCKET");
        }
    }

    #[test]
    fn test_env_var_substitution_with_default() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("TEST_UNSET_VAR_XYZ");
            std::env::set_var("TEST_SET_VAR_XYZ", "custom_val");
        }

        let substituted = substitute_env_vars(
            "host: ${TEST_UNSET_VAR_XYZ:-127.0.0.1}\nother: ${TEST_SET_VAR_XYZ:-default_val}",
        )
        .unwrap();

        assert_eq!(substituted, "host: 127.0.0.1\nother: custom_val");

        unsafe {
            std::env::remove_var("TEST_SET_VAR_XYZ");
        }
    }

    #[test]
    fn test_missing_env_var_error() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("TEST_MISSING_VAR_999");
        }

        let yaml = "account_id: \"${TEST_MISSING_VAR_999}\"";
        let result = parse_config_str(yaml);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("TEST_MISSING_VAR_999"));
    }

    #[test]
    fn test_unclosed_env_var_error() {
        let yaml = "account_id: \"${TEST_UNCLOSED_VAR\"";
        let result = parse_config_str(yaml);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Unclosed"));
    }

    #[test]
    fn test_empty_env_var_error() {
        let yaml = "account_id: \"${}\"";
        let result = parse_config_str(yaml);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Empty"));
    }

    #[test]
    fn test_escaped_placeholder() {
        let raw = "template: \"$${LITERAL_PLACEHOLDER}\"";
        let substituted = substitute_env_vars(raw).unwrap();
        assert_eq!(substituted, "template: \"${LITERAL_PLACEHOLDER}\"");
    }

    #[test]
    fn test_default_config_values() {
        let config = Config::default();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.admin_password, "change-me-in-production");
        assert_eq!(config.server.jwt_secret, None);
        assert_eq!(config.server.session_ttl_hours, 72);
        assert!(!config.server.headless);
        assert_eq!(config.database.url, "sqlite:///data/r2drive.db?mode=rwc");
        assert_eq!(config.sync.default_ttl_seconds, 60);
        assert_eq!(config.default_profile, "primary");
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn test_load_from_path() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"
server:
  port: 8888
database:
  url: "sqlite://test_load.db"
"#;
        file.write_all(content.as_bytes()).unwrap();

        let config = Config::load(Some(file.path())).unwrap();
        assert_eq!(config.server.port, 8888);
        assert_eq!(config.database.url, "sqlite://test_load.db");
        // Defaults preserved
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.sync.default_ttl_seconds, 60);
    }

    #[test]
    fn test_load_missing_file_error() {
        let result = Config::load(Some(Path::new("/path/that/does/not/exist/r2drive.yaml")));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found"));
    }

    #[test]
    fn test_load_from_env_r2drive_config() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"
server:
  port: 7777
"#;
        file.write_all(content.as_bytes()).unwrap();

        unsafe {
            std::env::set_var("R2DRIVE_CONFIG", file.path().to_str().unwrap());
        }

        let config = Config::load(None).unwrap();
        assert_eq!(config.server.port, 7777);

        unsafe {
            std::env::remove_var("R2DRIVE_CONFIG");
        }
    }

    #[test]
    fn test_commented_out_env_var_ignored() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("TEST_UNSET_VAR_COMMENT_1");
            std::env::remove_var("TEST_UNSET_VAR_COMMENT_2");
        }

        let yaml = r#"
# account_id: "${TEST_UNSET_VAR_COMMENT_1}"
#   Indented comment with ${TEST_UNSET_VAR_COMMENT_2}
server:
  port: 8080
default_profile: "primary"
profiles:
  primary:
    account_id: "acc_real"
    access_key_id: "key_real"
    secret_access_key: "secret_real"
    bucket_name: "bucket_real"
"#;
        let config = parse_config_str(yaml)
            .expect("Commented lines with unset variables should not trigger an error");
        assert_eq!(config.server.port, 8080);
        let profile = config.get_profile("primary").unwrap();
        assert_eq!(profile.account_id, "acc_real");

        let raw = "# key: ${TEST_UNSET_VAR_COMMENT_1}\nkey: static_val\n  # ${TEST_UNSET_VAR_COMMENT_2}";
        let substituted = substitute_env_vars(raw).unwrap();
        assert_eq!(substituted, raw);
    }

    #[test]
    fn test_config_from_str_trait() {
        let yaml = r#"
server:
  port: 9999
"#;
        let config: Config = yaml.parse().expect("Failed to parse via FromStr");
        assert_eq!(config.server.port, 9999);

        let config2 = Config::from_str(yaml).expect("Failed to parse via Config::from_str");
        assert_eq!(config2.server.port, 9999);
    }
}
