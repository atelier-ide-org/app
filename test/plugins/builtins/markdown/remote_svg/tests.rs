use super::*;

const BADGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="20"><rect width="80" height="20" fill="#8b39f6"/></svg>"##;

#[test]
fn only_the_addresses_the_other_loaders_refuse_are_taken() {
    assert!(handles("https://img.shields.io/badge/license-MIT-8b39f6"));
    assert!(handles("https://x.y/badge.svg?style=flat"));
    assert!(!handles("https://x.y/logo.svg"), "the SVG loader of egui");
    assert!(!handles("file:///C:/p/logo.png"));
    assert!(!handles("bytes://mermaid/1.png"));
}

#[test]
fn an_svg_is_known_by_what_the_server_says_or_by_its_text() {
    assert!(is_svg(Some("image/svg+xml;charset=utf-8"), b""));
    assert!(!is_svg(Some("image/png"), BADGE.as_bytes()));
    assert!(is_svg(None, BADGE.as_bytes()));
    assert!(is_svg(
        None,
        format!("\u{feff}<?xml version=\"1.0\"?>\n{BADGE}").as_bytes()
    ));
    assert!(!is_svg(None, b"\x89PNG\r\n\x1a\n"));
    assert!(!is_svg(None, b""));
}

#[test]
fn a_badge_is_drawn_at_the_size_asked() {
    let image = rasterize(BADGE.as_bytes(), SizeHint::Scale(2.0.into())).unwrap();
    assert_eq!(image.size, [160, 40]);
    assert_eq!(image.pixels[0], egui::Color32::from_rgb(0x8b, 0x39, 0xf6));
    let image = rasterize(BADGE.as_bytes(), SizeHint::Width(40)).unwrap();
    assert_eq!(image.size, [40, 10]);
    assert!(rasterize(b"not a picture", SizeHint::default()).is_err());
}
