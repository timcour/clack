use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_events_help() {
    let mut cmd = Command::cargo_bin("clack").unwrap();
    cmd.arg("events").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Listen to Slack events"));
}

#[test]
fn test_events_listen_help() {
    let mut cmd = Command::cargo_bin("clack").unwrap();
    cmd.arg("events").arg("listen").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--channel"))
        .stdout(predicate::str::contains("--from"));
}

#[test]
fn test_events_list_help() {
    let mut cmd = Command::cargo_bin("clack").unwrap();
    cmd.arg("events").arg("list").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--since"))
        .stdout(predicate::str::contains("--limit"));
}

#[test]
fn test_events_listen_requires_tokens() {
    // Without any tokens, the command should fail
    let mut cmd = Command::cargo_bin("clack").unwrap();
    cmd.env_remove("SLACK_APP_TOKEN")
        .env_remove("SLACK_TOKEN")
        .arg("events")
        .arg("listen");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("SLACK_TOKEN"));
}
