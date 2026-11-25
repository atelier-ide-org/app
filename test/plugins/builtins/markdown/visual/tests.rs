use super::*;

fn pieces(text: &str) -> Vec<&str> {
    blocks(text).into_iter().map(|r| &text[r]).collect()
}

#[test]
fn the_document_is_split_into_blocks_of_whole_lines() {
    let text = "# Titre\n\nUn paragraphe\nsur deux lignes.\n\n- a\n- b\n\n| A | B |\n| - | - |\n| 1 | 2 |\n\n[ref]: https://x.y\n";
    assert_eq!(
        pieces(text),
        [
            "# Titre",
            "Un paragraphe\nsur deux lignes.",
            "- a\n- b",
            "| A | B |\n| - | - |\n| 1 | 2 |",
            "[ref]: https://x.y",
        ]
    );
    assert_eq!(
        pieces("```rust\nfn a() {}\n```\n---\n"),
        ["```rust\nfn a() {}\n```", "---"]
    );
    assert!(blocks("").is_empty());
    assert!(blocks("\n\n  \n").is_empty());
}

#[test]
fn marks_wrap_the_selection_and_unwrap_it() {
    let mut text = "un mot ici".to_string();
    let selected = apply(&mut text, 3..6, Format::Wrap("**", "**"));
    assert_eq!(text, "un **mot** ici");
    assert_eq!(&text[selected.clone()], "mot");
    let selected = apply(&mut text, selected, Format::Wrap("**", "**"));
    assert_eq!(text, "un mot ici");
    assert_eq!(selected, 3..6);
    // Nothing selected: the caret between the marks.
    let mut text = "ab".to_string();
    assert_eq!(apply(&mut text, 1..1, Format::Wrap("`", "`")), 2..2);
    assert_eq!(text, "a``b");
}

#[test]
fn lines_get_a_heading_a_list_or_numbers() {
    let mut text = "Titre".to_string();
    apply(&mut text, 2..2, Format::Heading(2));
    assert_eq!(text, "## Titre");
    apply(&mut text, 2..2, Format::Heading(1));
    assert_eq!(text, "# Titre");
    apply(&mut text, 2..2, Format::Heading(1));
    assert_eq!(text, "Titre");

    let mut text = "a\nb".to_string();
    apply(&mut text, 0..3, Format::Prefix("- "));
    assert_eq!(text, "- a\n- b");
    let all = 0..text.len();
    apply(&mut text, all, Format::Numbered);
    assert_eq!(text, "1. a\n2. b");
    let all = 0..text.len();
    apply(&mut text, all, Format::Prefix("- [ ] "));
    assert_eq!(text, "- [ ] a\n- [ ] b");
    let all = 0..text.len();
    apply(&mut text, all, Format::Prefix("- [ ] "));
    assert_eq!(text, "a\nb");
}

#[test]
fn a_link_and_a_block_are_inserted() {
    let mut text = "voir ici".to_string();
    let selected = apply(&mut text, 5..8, Format::Link);
    assert_eq!(text, "voir [ici](https://)");
    assert_eq!(&text[selected], "https://");

    let mut text = "Texte".to_string();
    let caret = apply(&mut text, 2..2, Format::Insert(CODE));
    assert_eq!(text, "Texte\n\n```\n\n```");
    assert_eq!(caret, 11..11);
}

#[test]
fn the_source_is_styled_as_it_renders() {
    let t = crate::ui::theme_by_index(0);
    let job = highlight("# Titre **gras**\n- `code`", &t);
    assert_eq!(job.text, "# Titre **gras**\n- `code`");
    let big = job
        .sections
        .iter()
        .find(|s| {
            &job.text[usize::from(s.byte_range.start)..usize::from(s.byte_range.end)] == "Titre "
        })
        .unwrap();
    assert_eq!(big.format.font_id.size, 26.0);
    let code = job
        .sections
        .iter()
        .find(|s| {
            &job.text[usize::from(s.byte_range.start)..usize::from(s.byte_range.end)] == "code"
        })
        .unwrap();
    assert_eq!(code.format.font_id.family, egui::FontFamily::Monospace);
}

/// Runs `f` in a frame of a real context, the style of the rendering applied.
fn in_frame(f: impl FnMut(&mut egui::Ui)) {
    let ctx = egui::Context::default();
    let mut f = f;
    for _ in 0..2 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                preview_style(ui, &crate::ui::theme_by_index(0));
                f(ui);
            },
        );
        output.textures_delta.clear();
    }
}

#[test]
fn a_click_on_the_rendering_finds_its_place_in_the_source() {
    in_frame(|ui| {
        let source = "# Un titre";
        // At the very start and far to the right.
        assert_eq!(
            caret_at(ui, source, egui::vec2(0.0, 10.0), egui::vec2(700.0, 30.0)),
            2
        );
        assert_eq!(
            caret_at(ui, source, egui::vec2(690.0, 10.0), egui::vec2(700.0, 30.0)),
            10
        );

        // The marks of bold text are skipped: a click on `fin` is after them.
        let source = "Du **gras** et la fin";
        let measure = |text: &str, font: egui::FontId| {
            ui.fonts_mut(|f| {
                f.layout_no_wrap(text.into(), font, Default::default())
                    .rect
                    .width()
            })
        };
        let width = measure("Du ", egui::FontId::proportional(14.0))
            + measure("gras", crate::ui::semibold(14.0))
            + measure(" et la fi", egui::FontId::proportional(14.0));
        let caret = caret_at(
            ui,
            source,
            egui::vec2(width - 1.0, 8.0),
            egui::vec2(700.0, 18.0),
        );
        assert!(
            (18..=20).contains(&caret),
            "{caret}: {:?}",
            &source[caret..]
        );

        // The second item of a list.
        let source = "- un\n- deux";
        let caret = caret_at(ui, source, egui::vec2(60.0, 36.0), egui::vec2(700.0, 48.0));
        assert!((7..=11).contains(&caret), "{caret}");
    });
}
