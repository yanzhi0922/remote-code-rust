// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(all(feature = "desktop", not(any(target_os = "ios", target_os = "android"))))]
    {
        if let Err(err) = rc_codex_adapter::isolated_codex_home() {
            eprintln!("WARNING: failed to isolate CODEX_HOME for embedded Codex: {err}");
        }
        let _codex_arg0_guard = codex_arg0::arg0_dispatch();
        remote_code_gui_lib::run()
    }
    #[cfg(any(
        target_os = "ios",
        target_os = "android",
        not(feature = "desktop")
    ))]
    {
        remote_code_gui_lib::run()
    }
}
