fn main() {
    // Rerun this build script when the gating env var changes, so the
    // resource embedding toggle below takes effect without a full
    // `cargo clean`. This matters because winres 0.1.12's compile() emits
    // cargo:rustc-link-lib directives that, once cached, prevent the
    // script from being re-evaluated on subsequent builds unless we
    // explicitly opt in.
    println!("cargo:rerun-if-env-changed=REMOTE_CODE_GUI_BUILD");

    if std::env::var_os("RULES_RUST_BAZEL_BUILD_SCRIPT_RUNNER").is_some()
        && matches!(std::env::var("CARGO_CFG_TARGET_ENV").as_deref(), Ok("gnu"))
    {
        // The Windows Bazel lint/test lane targets `windows-gnullvm`, where
        // `winres` can emit a `resource` link directive without a usable
        // archive in `OUT_DIR`. Skip embedding the manifest there; Cargo's
        // normal MSVC builds still compile it.
        return;
    }

    // REMOTE_CODE_PATCH: skip winres resource embedding when this crate is
    // being linked into the remote-code-gui Tauri binary. The SxS manifest
    // is only meaningful for the standalone `codex-windows-sandbox-setup.exe`
    // (which requests UAC elevation to launch the sandbox); when this
    // crate is consumed as a library by `remote-code-gui`, the embedded
    // VERSIONINFO resource (winres 0.1 hardcodes ID=1) collides with
    // tauri-build's own VERSIONINFO and triggers CVT1100 at link time.
    if std::env::var_os("REMOTE_CODE_GUI_BUILD").is_some() {
        return;
    }

    let mut res = winres::WindowsResource::new();
    res.set_manifest_file("codex-windows-sandbox-setup.manifest");
    let _ = res.compile();
}
