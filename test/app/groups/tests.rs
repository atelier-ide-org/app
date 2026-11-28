use super::*;

#[test]
fn the_side_under_the_pointer_is_the_middle_or_the_nearest_edge() {
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0));
    assert_eq!(Side::at(rect, pos2(50.0, 50.0)), Side::Center);
    assert_eq!(Side::at(rect, pos2(95.0, 50.0)), Side::Right);
    assert_eq!(Side::at(rect, pos2(5.0, 40.0)), Side::Left);
    assert_eq!(Side::at(rect, pos2(50.0, 95.0)), Side::Bottom);
    assert_eq!(Side::at(rect, pos2(40.0, 4.0)), Side::Top);
}

#[test]
fn a_dropped_tab_takes_the_half_of_its_side() {
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 80.0));
    assert_eq!(
        Side::Right.area(rect),
        Rect::from_min_max(pos2(50.0, 0.0), pos2(100.0, 80.0))
    );
    assert_eq!(
        Side::Top.area(rect),
        Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 40.0))
    );
    assert_eq!(Side::Center.area(rect), rect);
}

/// An IDE on a new folder with `names` open, each an empty file.
fn ide_with(test: &str, names: &[&str]) -> (Ide, Vec<PathBuf>) {
    let dir = std::env::temp_dir().join(format!("atelier-groups-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    let mut ide = Ide::blank(
        dir.clone(),
        super::super::Settings {
            font_size: 14.0,
            ..Default::default()
        },
        None,
    );
    let paths: Vec<PathBuf> = names
        .iter()
        .map(|name| {
            let path = dir.join(name);
            std::fs::write(&path, "").unwrap();
            ide.open_file(path.clone());
            path
        })
        .collect();
    (ide, paths)
}

/// What drawing a window does: its layout is out of its place while its tabs are drawn, and a
/// tab closed then empties its group.
fn close_while_drawn(ide: &mut Ide, window: Option<usize>, group: u64) {
    let layout = match window {
        None => std::mem::replace(&mut ide.editor_layout, Node::Empty),
        Some(w) => std::mem::replace(&mut ide.windows[w].layout, Node::Empty),
    };
    ide.take_tab(group, 0);
    match window {
        None => ide.editor_layout = layout,
        Some(w) => ide.windows[w].layout = layout,
    }
}

#[test]
fn closing_the_last_tab_of_a_window_closes_the_window_not_the_ide() {
    let (mut ide, paths) = ide_with("window", &["a.txt"]);
    ide.open_window_with(paths[0].clone(), 0, None);
    let group = *ide.windows[0].layout.leaves()[0];
    // Panicked: the window went away while it was drawn.
    close_while_drawn(&mut ide, Some(0), group);
    ide.prune_groups();
    assert!(ide.windows.is_empty());
    assert!(ide.groups.is_empty());
    assert_eq!(
        ide.documents.len(),
        1,
        "the file stays open in the main window"
    );
}

#[test]
fn closing_the_last_tab_of_a_split_leaves_no_empty_island() {
    let (mut ide, _) = ide_with("split", &["a.txt"]);
    ide.split_to(false);
    let group = **ide
        .editor_layout
        .leaves()
        .iter()
        .find(|g| ***g != MAIN)
        .unwrap();
    close_while_drawn(&mut ide, None, group);
    ide.prune_groups();
    assert!(
        !ide.has_splits(),
        "the main group takes the whole editor again"
    );
    assert!(ide.groups.is_empty());
}

#[test]
fn a_window_goes_back_beside_the_main_group_as_it_was() {
    let (mut ide, paths) = ide_with("attach", &["a.txt", "b.txt"]);
    ide.open_window_with(paths[0].clone(), 0, None);
    let first = *ide.windows[0].layout.leaves()[0];
    ide.split_group(first, paths[1].clone(), 0, Side::Bottom);
    assert_eq!(ide.windows[0].layout.leaves().len(), 2);
    ide.attach_window(first);
    assert!(ide.windows.is_empty());
    let leaves: Vec<u64> = ide.editor_layout.leaves().into_iter().copied().collect();
    assert_eq!(leaves.len(), 3, "the main group and the two of the window");
    assert_eq!(leaves[0], MAIN);
    assert_eq!(ide.groups.len(), 2);
}

#[test]
fn a_tab_dragged_out_of_its_window_lands_in_the_window_under_the_pointer() {
    let (mut ide, paths) = ide_with("cross", &["a.txt"]);
    ide.open_window_with(paths[0].clone(), 0, None);
    let window = ide.windows[0].id;
    let group = *ide.windows[0].layout.leaves()[0];
    // The main window at (100, 100) on the screen, its main group 800 × 600 at (0, 40) in it;
    // the other window at (1200, 100), its group filling 600 × 400.
    ide.viewport_origins
        .insert(ViewportId::ROOT, pos2(100.0, 100.0));
    ide.drop_targets.insert(
        ViewportId::ROOT,
        vec![(
            MAIN,
            Rect::from_min_size(pos2(0.0, 40.0), vec2(800.0, 600.0)),
        )],
    );
    ide.viewport_origins.insert(window, pos2(1200.0, 100.0));
    ide.drop_targets.insert(
        window,
        vec![(
            group,
            Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 400.0)),
        )],
    );
    // Dragged out of the other window, over the middle of the main group.
    assert_eq!(
        ide.window_target(pos2(500.0, 440.0), window),
        Some((ViewportId::ROOT, MAIN, Side::Center))
    );
    // Over the right edge of the other window's group, dragged out of the main window.
    assert_eq!(
        ide.window_target(pos2(1790.0, 300.0), ViewportId::ROOT),
        Some((window, group, Side::Right))
    );
    // Over neither: a window of its own.
    assert_eq!(ide.window_target(pos2(1000.0, 900.0), window), None);
    // Its own window is not a target.
    assert_eq!(ide.window_target(pos2(1500.0, 300.0), window), None);
}
