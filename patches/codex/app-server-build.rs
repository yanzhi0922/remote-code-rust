fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_env == "msvc" {
        println!("cargo:rustc-link-arg-bin=codex-app-server=/STACK:16777216");
        println!("cargo:rustc-link-arg-tests=/STACK:16777216");
    } else {
        println!("cargo:rustc-link-arg-bin=codex-app-server=-Wl,--stack,16777216");
        println!("cargo:rustc-link-arg-tests=/STACK:16777216");
    }
}
