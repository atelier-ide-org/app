use super::*;

#[test]
fn svg_files_get_a_split_layout_by_default() {
    let mut plugin = SvgPlugin::default();
    let logo = Path::new("C:/projet/logo.SVG");
    assert_eq!(plugin.editor_layout(logo), Some(EditorLayout::Split));
    assert_eq!(plugin.editor_layout(Path::new("main.rs")), None);
    plugin.set_editor_layout(logo, EditorLayout::Preview);
    assert_eq!(plugin.editor_layout(logo), Some(EditorLayout::Preview));
}

#[test]
fn preview_renders() {
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    let mut plugin = SvgPlugin::default();
    let mut text = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><circle cx="10" cy="10" r="8"/></svg>"#.to_string();
    for _ in 0..2 {
        ctx.run_ui(Default::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                plugin.preview(ui, Path::new("a.svg"), &mut text, None);
            });
        })
        .drop_without_applying_deltas();
    }
    assert_eq!(plugin.uris.len(), 1);
}
