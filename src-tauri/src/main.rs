#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The statusline hook launches this same binary many times a minute; the
    // ingest flag must be caught here, before quotos_app_lib::run() touches
    // Tauri, or each invocation would spin up a second GUI instance.
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 4 && args[1] == quotos_app_lib::statusline::INGEST_FLAG {
        std::process::exit(quotos_app_lib::statusline::run_ingest_from_stdin(
            &args[2], &args[3],
        ));
    }
    quotos_app_lib::run()
}
