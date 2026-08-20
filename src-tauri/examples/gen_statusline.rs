//! Scratch tool for the bounded live check in the statusline v2 PR: drives
//! `statusline::enable`/`disable` against caller-supplied directories so the
//! live check exercises the real generator, not a hand-written script. Not
//! part of the shipped app.

use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args
        .next()
        .expect("usage: <enable|disable> <app_support_dir> <config_dir>");
    let app_support_dir = PathBuf::from(
        args.next()
            .expect("usage: <enable|disable> <app_support_dir> <config_dir>"),
    );
    let config_dir = PathBuf::from(
        args.next()
            .expect("usage: <enable|disable> <app_support_dir> <config_dir>"),
    );

    match mode.as_str() {
        "enable" => {
            quotos_app_lib::statusline::enable(&app_support_dir, &config_dir)
                .expect("enable failed");
        }
        "disable" => {
            quotos_app_lib::statusline::disable(&app_support_dir, &config_dir)
                .expect("disable failed");
        }
        other => panic!("unknown mode {other}, expected enable or disable"),
    }
    println!("{mode} done for {}", config_dir.display());
}
