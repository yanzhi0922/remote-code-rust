use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn fake_provider_runs_single_shot_without_network() {
    let workspace = tempfile::tempdir().expect("temp workspace");

    Command::cargo_bin("roo")
        .expect("roo binary")
        .args([
            "--provider",
            "fake-ai",
            "--working-dir",
            workspace.path().to_str().expect("utf-8 temp path"),
            "--message",
            "hello",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello from FakeAI"));
}

#[test]
fn config_file_can_override_fake_response() {
    let workspace = tempfile::tempdir().expect("temp workspace");
    let config_path = workspace.path().join("roo-config.json");
    std::fs::write(
        &config_path,
        r#"{"provider":"fake-ai","fake_response":"scripted fake reply"}"#,
    )
    .expect("write config");

    Command::cargo_bin("roo")
        .expect("roo binary")
        .args([
            "--config",
            config_path.to_str().expect("utf-8 config path"),
            "--working-dir",
            workspace.path().to_str().expect("utf-8 temp path"),
            "--message",
            "hello",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("scripted fake reply"));
}
