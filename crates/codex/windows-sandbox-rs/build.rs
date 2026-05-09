fn main() {
    if std::env::var_os("RULES_RUST_BAZEL_BUILD_SCRIPT_RUNNER").is_some()
        && matches!(std::env::var("CARGO_CFG_TARGET_ENV").as_deref(), Ok("gnu"))
    {
        // The Windows Bazel lint/test lane targets `windows-gnullvm`, where
        // `winres` can emit a `resource` link directive without a usable
        // archive in `OUT_DIR`. Skip embedding the manifest there; Cargo's
        // normal MSVC builds still compile it.
        return;
    }

    // Only embed the Windows resource for the setup binary.
    // The lib target is used by downstream GUI binaries (e.g. Tauri) that
    // already embed their own VERSION resource via `tauri_build`, causing
    // CVTRES CVT1100 "duplicate resource" when both are linked.
    if std::env::var("CARGO_BIN_NAME").is_ok() {
        let mut res = winres::WindowsResource::new();
        res.set_manifest_file("codex-windows-sandbox-setup.manifest");
        let _ = res.compile();
    }
}
