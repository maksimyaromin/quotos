// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The statusline helper is a copy of this very executable (see
    // statusline.rs's `ensure_helper_installed`) invoked by Claude Code's
    // statusline hook, possibly many times a minute during an active
    // session. Intercepting the sentinel here — before `quotos_app_lib::run()`
    // touches Tauri at all — is what keeps that from ever spinning up a
    // second GUI instance.
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 4 && args[1] == quotos_app_lib::statusline::INGEST_FLAG {
        std::process::exit(quotos_app_lib::statusline::run_ingest_from_stdin(
            &args[2], &args[3],
        ));
    }
    quotos_app_lib::run()
}
