use std::process::Command;

use tempfile::tempdir;

#[test]
fn doctor_outputs_runner_summary() {
    let profile_dir = tempdir().expect("tempdir should exist");
    let output = Command::new(env!("CARGO_BIN_EXE_remote-code-runner"))
        .args([
            "--runner-id",
            "runner-cli",
            "--control-plane-url",
            "http://127.0.0.1:8787",
            "--profile-dir",
            profile_dir
                .path()
                .to_str()
                .expect("profile dir should be utf-8"),
            "doctor",
        ])
        .output()
        .expect("runner doctor should execute");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("json should parse");
    assert_eq!(json["runner_id"], "runner-cli");
    assert_eq!(json["ok"], true);
    assert_eq!(json["phase"], "phase4-remote-beta");
}
