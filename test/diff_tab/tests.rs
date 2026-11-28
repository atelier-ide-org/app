use super::*;

fn tab(old: &str, new: &str) -> DiffTab {
    DiffTab::new(DiffRequest {
        path: "src/main.rs".into(),
        left_title: "abc1234".into(),
        right_title: "def5678".into(),
        old: old.into(),
        new: new.into(),
    })
}

#[test]
fn each_side_shows_its_file_with_its_changes() {
    // b is changed into B, d is added, e is removed.
    let tab = tab("a\nb\nc\ne\n", "a\nB\nc\nd\n");
    assert_eq!(tab.old.text, "a\nb\nc\ne\n");
    assert_eq!(tab.new.text, "a\nB\nc\nd\n");
    assert_eq!(tab.title(), "main.rs (abc1234 ↔ def5678)");
    assert_eq!(tab.language(), "rs");
    assert_eq!(tab.old.kinds, [(1, Kind::Changed), (3, Kind::Changed)]);
    assert_eq!(tab.new.kinds, [(1, Kind::Changed), (3, Kind::Changed)]);
}

#[test]
fn added_and_removed_lines_are_marked_on_their_side_only() {
    let tab = tab("a\nb\nc\n", "a\nc\nd\n");
    assert_eq!(tab.old.kinds, [(1, Kind::Deleted)]);
    assert_eq!(tab.new.kinds, [(2, Kind::Inserted)]);
    assert_eq!(tab.starts.len(), 2, "two changes to go through with F7");
}

#[test]
fn the_other_side_scrolls_to_the_same_lines() {
    // Three lines inserted after the first one: line 1 of the old file is line 4 of the new.
    let tab = tab("a\nb\nc\n", "a\nx\ny\nz\nb\nc\n");
    assert_eq!(tab.across(true, 0.0), 0.0);
    assert_eq!(tab.across(true, 1.0), 4.0);
    assert_eq!(
        tab.across(true, 1.5),
        4.5,
        "the part of a line scrolled past follows"
    );
    assert_eq!(tab.across(false, 4.0), 1.0);
    // An inserted line of the new file faces the line that comes after it in the old one.
    assert_eq!(tab.across(false, 2.0), 1.0);
    // Past the end: the end of the other file.
    assert!(tab.across(true, 50.0) <= 6.0);
}

#[test]
fn the_unified_view_puts_removed_lines_above_their_replacement() {
    let tab = tab("a\nb\nc\n", "a\nB\nc\n");
    assert_eq!(tab.unified.text, "a\nb\nB\nc");
    assert_eq!(tab.unified.kinds, [(1, Kind::Deleted), (2, Kind::Inserted)]);
    // The line of each row in the old and the new file.
    assert_eq!(tab.unified.numbers, "1 1\n2  \n  2\n3 3");
    assert_eq!(tab.unified.starts, [1]);
}

#[test]
fn hiding_identical_lines_folds_them_in_the_unified_view() {
    let old: String = (1..=30).map(|i| format!("line {i}\n")).collect();
    let new = old.replace("line 15\n", "line fifteen\n");
    let mut tab = tab(&old, &new);
    let full = tab.unified.text.lines().count();
    tab.collapse = true;
    tab.build_unified();
    let folded = tab.unified.text.lines().count();
    assert!(folded < full, "{folded} lines, {full} before");
    assert!(tab.unified.text.contains('⋯'));
}

#[test]
fn a_binary_file_is_not_compared() {
    let tab = tab("a\0b", "a\0c");
    assert!(tab.binary);
}

#[test]
fn each_change_links_its_lines_on_both_sides() {
    // Line 1 replaced, two lines inserted after line 3, line 5 removed.
    let tab = tab("a\nb\nc\nd\ne\nf\n", "a\nB\nc\nx\ny\nd\nf\n");
    assert_eq!(
        tab.blocks,
        [
            Block {
                old: 1..2,
                new: 1..2,
                kind: Kind::Changed
            },
            Block {
                old: 3..3,
                new: 3..5,
                kind: Kind::Inserted
            },
            Block {
                old: 4..5,
                new: 6..6,
                kind: Kind::Deleted
            },
        ]
    );
}

#[test]
fn a_change_at_the_end_links_to_the_end_of_the_other_file() {
    let tab = tab("a\nb\n", "a\nb\nc\nd\n");
    assert_eq!(
        tab.blocks,
        [Block {
            old: 2..2,
            new: 2..4,
            kind: Kind::Inserted
        }]
    );
}
