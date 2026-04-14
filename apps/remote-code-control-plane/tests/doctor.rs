use std::fs;
use std::process::Command;

#[test]
fn doctor_outputs_control_plane_summary() {
    let profile_dir = std::env::temp_dir().join(format!(
        "remote-code-control-plane-test-{}",
        std::process::id()
    ));
    fs::create_dir_all(&profile_dir).expect("temp profile dir should exist");

    let output = Command::new(env!("CARGO_BIN_EXE_remote-code-control-plane"))
        .args([
            "--bind",
            "127.0.0.1:9899",
            "--profile-dir",
            profile_dir.to_str().expect("temp path should be utf-8"),
            "--service-name",
            "control-plane-test",
            "doctor",
        ])
        .output()
        .expect("control-plane doctor should execute");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("json should parse");
    assert_eq!(json["service_name"], "control-plane-test");
    assert_eq!(json["bind"], "127.0.0.1:9899");
    assert_eq!(json["phase"], "phase5-remote-stable");
}
