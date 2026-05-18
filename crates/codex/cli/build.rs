fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        println!("cargo:rustc-link-arg=-ObjC");
    }

    if target_os == "windows" {
        let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
        if target_env == "msvc" {
            println!("cargo:rustc-link-arg-bin=codex=/STACK:16777216");
        } else {
            println!("cargo:rustc-link-arg-bin=codex=-Wl,--stack,16777216");
        }
    }
}
