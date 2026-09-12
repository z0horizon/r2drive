use assert_cmd::Command;
use assert_cmd::cargo::CommandCargoExt;
use predicates::prelude::*;

#[test]
fn test_cli_help_flag() {
    let mut cmd = Command::cargo_bin("r2drive").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("r2drive"))
        .stdout(predicate::str::contains("ls"))
        .stdout(predicate::str::contains("upload"))
        .stdout(predicate::str::contains("download"))
        .stdout(predicate::str::contains("cat"))
        .stdout(predicate::str::contains("rm"))
        .stdout(predicate::str::contains("serve"));
}

#[test]
fn test_cli_version_flag() {
    let mut cmd = Command::cargo_bin("r2drive").unwrap();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_cli_invalid_subcommand() {
    let mut cmd = Command::cargo_bin("r2drive").unwrap();
    cmd.arg("nonexistent-subcommand-xyz");
    cmd.assert().failure();
}

#[test]
fn test_cli_missing_required_args_download() {
    let mut cmd = Command::cargo_bin("r2drive").unwrap();
    cmd.arg("download");
    cmd.assert().failure();
}

#[test]
fn test_cli_missing_required_args_upload() {
    let mut cmd = Command::cargo_bin("r2drive").unwrap();
    cmd.arg("upload");
    cmd.assert().failure();
}

#[test]
fn test_cli_missing_required_args_cat() {
    let mut cmd = Command::cargo_bin("r2drive").unwrap();
    cmd.arg("cat");
    cmd.assert().failure();
}

#[test]
fn test_cli_missing_required_args_rm() {
    let mut cmd = Command::cargo_bin("r2drive").unwrap();
    cmd.arg("rm");
    cmd.assert().failure();
}

#[test]
fn test_cli_serve_help() {
    let mut cmd = Command::cargo_bin("r2drive").unwrap();
    cmd.args(["serve", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("port"))
        .stdout(predicate::str::contains("headless"));
}

#[test]
fn test_cli_subcommands_help() {
    for subcommand in ["ls", "upload", "download", "cat", "rm", "serve"] {
        let mut cmd = Command::cargo_bin("r2drive").unwrap();
        cmd.args([subcommand, "--help"]);
        cmd.assert().success();
    }
}

#[test]
fn test_cli_serve_execution() {
    use std::io::Write;
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    let mut temp_file = tempfile::NamedTempFile::new().unwrap();
    let yaml = r#"
server:
  host: "127.0.0.1"
  port: 18088
  admin_password: "test-password"
database:
  url: "sqlite::memory:"
profiles:
  primary:
    account_id: "0123456789abcdef0123456789abcdef"
    access_key_id: "test-access-key"
    secret_access_key: "test-secret-key"
    bucket_name: "test-bucket"
"#;
    temp_file.write_all(yaml.as_bytes()).unwrap();

    let mut cmd = std::process::Command::cargo_bin("r2drive").unwrap();
    cmd.args([
        "-c",
        temp_file.path().to_str().unwrap(),
        "serve",
        "--headless",
        "-P",
        "18088",
    ]);

    let mut child = cmd.spawn().expect("Failed to spawn r2drive serve");

    let start = Instant::now();
    let mut connected = false;
    while start.elapsed() < Duration::from_secs(5) {
        if TcpStream::connect("127.0.0.1:18088").is_ok() {
            connected = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(
        connected,
        "Server did not accept connections on port 18088 within timeout"
    );
}

#[test]
fn test_cli_serve_missing_config_fails() {
    let mut cmd = Command::cargo_bin("r2drive").unwrap();
    cmd.args([
        "-c",
        "/path/that/does/not/exist.yaml",
        "serve",
        "--headless",
        "-P",
        "8088",
    ]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Configuration file not found"));
}

#[test]
fn test_cli_missing_config_fails_gracefully() {
    let mut cmd = Command::cargo_bin("r2drive").unwrap();
    cmd.args(["-c", "/path/that/does/not/exist.yaml", "ls"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Configuration file not found"));
}
