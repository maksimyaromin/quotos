//! Renders the tray scenes the design mockup shows, side by side, so a
//! change to `status_item_render` can be looked at rather than only
//! asserted about. `cargo run --example dump_tray_scenes -- <out dir>`.

use quotos_app_lib::status_item_render::{GroupColor, StatusItemColor, StatusItemSegment, render};

fn fig(text: &str, color: StatusItemColor) -> StatusItemSegment {
    StatusItemSegment::Figure {
        text: text.into(),
        color,
    }
}

fn chip(slug: &str, color: GroupColor) -> StatusItemSegment {
    StatusItemSegment::Chip {
        slug: slug.into(),
        color,
    }
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let dark = std::env::var_os("QUOTOS_DUMP_LIGHT").is_none();

    let scenes: Vec<(&str, Vec<StatusItemSegment>)> = vec![
        (
            "no-groups",
            vec![
                fig("18%", StatusItemColor::Neutral),
                fig("55%", StatusItemColor::Neutral),
                fig("99%", StatusItemColor::Red),
                fig("47%", StatusItemColor::Neutral),
            ],
        ),
        (
            "collapsed-chips",
            vec![
                chip("FAB", GroupColor::Blue),
                chip("CUR", GroupColor::Red),
                fig("12%", StatusItemColor::Neutral),
            ],
        ),
        (
            "doc-menu-bar",
            vec![
                chip("FAB", GroupColor::Blue),
                fig("18%", StatusItemColor::Neutral),
                fig("55%", StatusItemColor::Neutral),
                chip("CUR", GroupColor::Red),
                fig("12%", StatusItemColor::Neutral),
            ],
        ),
        (
            "opened-chip",
            vec![
                chip("FAB", GroupColor::Blue),
                chip("CUR", GroupColor::Red),
                fig("99%", StatusItemColor::Red),
                fig("47%", StatusItemColor::Neutral),
                fig("12%", StatusItemColor::Neutral),
            ],
        ),
    ];

    for (name, segments) in scenes {
        let (rgba, w, h) = render(&segments, false, 55, dark);
        let path = format!("{out}/{name}.rgba");
        std::fs::write(&path, &rgba).expect("write the bitmap");
        println!("{name} {w}x{h} -> {path}");
    }
}
