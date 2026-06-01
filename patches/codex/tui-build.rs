fn main() {
    if let Some(codex_root) = codex_workspace_root() {
        println!(
            "cargo:rustc-env=INSTA_WORKSPACE_ROOT={}",
            codex_root.display()
        );
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_env == "msvc" {
        println!("cargo:rustc-link-arg-bin=codex-tui=/STACK:16777216");
        println!("cargo:rustc-link-arg-bin=md-events=/STACK:16777216");
        println!("cargo:rustc-link-arg-tests=/STACK:16777216");
    } else {
        println!("cargo:rustc-link-arg-bin=codex-tui=-Wl,--stack,16777216");
        println!("cargo:rustc-link-arg-bin=md-events=-Wl,--stack,16777216");
        println!("cargo:rustc-link-arg-tests=/STACK:16777216");
    }
}

fn codex_workspace_root() -> Option<std::path::PathBuf> {
    let manifest_dir = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR")?);
    let mut workspace_root = None;
    for ancestor in manifest_dir.ancestors() {
        if ancestor.join("Cargo.toml").is_file() {
            workspace_root = Some(ancestor.to_path_buf());
        }
    }
    workspace_root
}
