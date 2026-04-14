use std::process::Command;

#[test]
fn doctor_outputs_control_plane_summary() {
    let output = Command::new(env!("CARGO_BIN_EXE_remote-code-control-plane"))
        .args([
            "--bind",
            "127.0.0.1:9899",
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
    assert_eq!(json["phase"], "phase4-remote-beta");
}
