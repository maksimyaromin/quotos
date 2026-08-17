#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 4 && args[1] == quotos_app_lib::statusline::INGEST_FLAG {
        std::process::exit(quotos_app_lib::statusline::run_ingest_from_stdin(
            &args[2], &args[3],
        ));
    }
    quotos_app_lib::run()
}
