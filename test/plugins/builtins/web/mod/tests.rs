use super::*;

#[test]
fn the_address_bar_opens_addresses_sites_files_and_searches() {
    assert_eq!(address("https://example.com/a"), "https://example.com/a");
    assert_eq!(address("about:blank"), "about:blank");
    assert_eq!(address("example.com"), "https://example.com");
    assert_eq!(address("docs.rs/egui?x=1"), "https://docs.rs/egui?x=1");
    assert_eq!(address("localhost:3000"), "http://localhost:3000");
    assert_eq!(address("127.0.0.1:8080/api"), "http://127.0.0.1:8080/api");
    assert_eq!(address("myhost:8000"), "https://myhost:8000");
    assert_eq!(address("rust egui"), "https://duckduckgo.com/?q=rust+egui");
    assert_eq!(address("egui"), "https://duckduckgo.com/?q=egui");
    assert_eq!(
        address("c++ & co"),
        "https://duckduckgo.com/?q=c%2B%2B+%26+co"
    );
    assert_eq!(address("  "), "about:blank");
    assert_eq!(
        address(r"C:\site\my page.html"),
        "file:///C:/site/my%20page.html"
    );
}

#[test]
fn file_addresses_leave_out_the_prefix_of_canonical_paths() {
    assert_eq!(
        file_url(Path::new(r"\\?\C:\a\index.html")),
        "file:///C:/a/index.html"
    );
    assert_eq!(
        file_url(Path::new(r"\\?\UNC\server\share\a.html")),
        "file://server/share/a.html"
    );
    assert_eq!(
        file_url(Path::new("/home/a/b#c.html")),
        "file:///home/a/b%23c.html"
    );
}

#[test]
fn pages_are_internet_shortcuts() {
    let text = write_shortcut("https://example.com/");
    assert_eq!(
        read_shortcut(&text).as_deref(),
        Some("https://example.com/")
    );
    // As Windows writes them, with more keys.
    let windows = "[{000214A0-0000-0000-C000-000000000046}]\r\nProp3=19,11\r\n[InternetShortcut]\r\nIDList=\r\nURL=https://www.rust-lang.org/\r\n";
    assert_eq!(
        read_shortcut(windows).as_deref(),
        Some("https://www.rust-lang.org/")
    );
    assert_eq!(read_shortcut("[InternetShortcut]\n"), None);
}

#[test]
fn html_files_show_their_page_on_demand_and_shortcuts_always() {
    let mut plugin = WebPlugin::default();
    let html = Path::new("site/index.html");
    let shortcut = Path::new("docs.url");
    assert_eq!(plugin.editor_layout(html), Some(EditorLayout::Text));
    assert!(plugin.layout_switch(html) && !plugin.opens_binary(html));
    plugin.set_editor_layout(html, EditorLayout::Split);
    assert_eq!(plugin.editor_layout(html), Some(EditorLayout::Split));
    assert_eq!(plugin.editor_layout(shortcut), Some(EditorLayout::Preview));
    assert!(plugin.opens_binary(shortcut) && !plugin.layout_switch(shortcut));
    assert_eq!(plugin.editor_layout(Path::new("a.css")), None);
}
