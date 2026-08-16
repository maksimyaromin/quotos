// windows_subsystem = "windows" stops Windows from opening a console window
// alongside the app in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The statusline hook invokes a copy of this executable directly, so this
    // sentinel must be intercepted before quotos_app_lib::run() touches Tauri.
    // Otherwise each hook invocation spins up a second GUI instance.
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 4 && args[1] == quotos_app_lib::statusline::INGEST_FLAG {
        std::process::exit(quotos_app_lib::statusline::run_ingest_from_stdin(
            &args[2], &args[3],
        ));
    }
    quotos_app_lib::run()
}
