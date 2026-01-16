use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;

#[test]
fn test_missing_slack_token() {
    // Ensure SLACK_TOKEN is not set for this test
    let mut cmd = cargo_bin_cmd!("clack");
    cmd.env_remove("SLACK_TOKEN")
        .arg("users")
        .assert()
        .failure()
        .stderr(predicate::str::contains("SLACK_TOKEN environment variable not set"));
}

#[test]
fn test_help_output() {
    let mut cmd = cargo_bin_cmd!("clack");
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("A Slack API CLI tool"))
        .stdout(predicate::str::contains("users"))
        .stdout(predicate::str::contains("messages"));
}

#[test]
fn test_version_output() {
    let mut cmd = cargo_bin_cmd!("clack");
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("clack"))
        .stdout(predicate::str::contains("0.1.0"));
}

#[test]
fn test_users_command_help() {
    let mut cmd = cargo_bin_cmd!("clack");
    cmd.arg("users")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("List all users"))
        .stdout(predicate::str::contains("--limit"))
        .stdout(predicate::str::contains("--include-deleted"));
}

#[test]
fn test_user_command_help() {
    let mut cmd = cargo_bin_cmd!("clack");
    cmd.arg("user")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Get a specific user by ID"))
        .stdout(predicate::str::contains("<USER_ID>"));
}

#[test]
fn test_messages_command_help() {
    let mut cmd = cargo_bin_cmd!("clack");
    cmd.arg("messages")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("List messages from a channel"))
        .stdout(predicate::str::contains("--limit"))
        .stdout(predicate::str::contains("--latest"))
        .stdout(predicate::str::contains("--oldest"));
}

#[test]
fn test_invalid_command() {
    let mut cmd = cargo_bin_cmd!("clack");
    cmd.arg("invalid-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn test_user_command_missing_argument() {
    let mut cmd = cargo_bin_cmd!("clack");
    cmd.arg("user")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_messages_command_missing_argument() {
    let mut cmd = cargo_bin_cmd!("clack");
    cmd.arg("messages")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}
