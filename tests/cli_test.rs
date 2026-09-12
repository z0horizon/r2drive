use assert_cmd::Command;
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
    let mut cmd = Command::cargo_bin("r2drive").unwrap();
    cmd.args(["serve", "--headless", "-P", "8088"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Server will be implemented in Task 6"))
        .stdout(predicate::str::contains("8088"));
}

#[test]
fn test_cli_missing_config_fails_gracefully() {
    let mut cmd = Command::cargo_bin("r2drive").unwrap();
    cmd.args(["-c", "/path/that/does/not/exist.yaml", "ls"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Configuration file not found"));
}
