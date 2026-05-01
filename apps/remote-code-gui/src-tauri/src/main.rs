// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        let _codex_arg0_guard = match rc_codex_adapter::isolated_codex_home() {
            Ok(codex_home) => codex_arg0::arg0_dispatch_with_codex_home(&codex_home),
            Err(err) => {
                eprintln!("FATAL: failed to isolate CODEX_HOME for embedded Codex: {err}");
                std::process::exit(1);
            }
        };
        remote_code_gui_lib::run()
    }
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        remote_code_gui_lib::run()
    }
}
