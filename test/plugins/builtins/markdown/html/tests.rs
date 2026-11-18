use super::*;

#[test]
fn tags_of_a_paragraph_become_markdown() {
    let source = "Press <kbd>Ctrl</kbd> and <b>save</b><br>See <a href=\"doc.md\">the doc</a> <img src=\"a.png\" alt=\"A\">\n";
    let converted = inline_to_markdown(source).unwrap();
    assert_eq!(
        converted.text,
        "Press `Ctrl` and **save**\\\nSee [the doc](<doc.md>) ![A](<a.png>)\n"
    );
    assert!(inline_to_markdown("# Plain *Markdown*\n").is_none());
    // Code keeps its tags.
    assert!(inline_to_markdown("`<b>` and\n\n```\n<br>\n```\n").is_none());
}

#[test]
fn offsets_of_the_converted_text_map_back_to_the_source() {
    let source = "- [ ] <b>a</b>\n- [ ] b\n";
    let converted = inline_to_markdown(source).unwrap();
    let box_at = converted.text.rfind("[ ]").unwrap() + 1;
    assert_eq!(
        converted.source_offset(box_at),
        source.rfind("[ ]").unwrap() + 1
    );
}

#[test]
fn a_centered_block_keeps_its_alignment_and_images() {
    let lines = layout(
        "<p align=\"center\">\n  <a href=\"https://x.y\"><img src=\"logo.png\" width=\"120\"></a>\n</p>\n<h1 align=\"center\">Atelier &amp; co</h1>\n<!-- hidden -->\n<hr>\n<ul><li>one</li><li>two</li></ul>",
    );
    assert_eq!(lines.len(), 5);
    let Line::Content { align, items, .. } = &lines[0] else {
        panic!()
    };
    assert_eq!(*align, Align::Center);
    assert!(matches!(
        &items[0],
        Inline::Image { src, width: Some(w), link: Some(l), .. }
            if src == "logo.png" && *w == 120.0 && l == "https://x.y"
    ));
    let Line::Content { heading, items, .. } = &lines[1] else {
        panic!()
    };
    assert_eq!(*heading, Some(1));
    assert!(matches!(&items[0], Inline::Text { text, strong: true, .. } if text == "Atelier & co"));
    assert_eq!(lines[2], Line::Rule);
    assert!(matches!(
        &lines[4],
        Line::Content { indent: 1, items, .. }
            if matches!(&items[0], Inline::Text { text, .. } if text == "• two")
    ));
}

#[test]
fn entities_are_decoded() {
    assert_eq!(
        decode_entities("a &lt;b&gt; &#39;c&#x27; &unknown; &"),
        "a <b> 'c' &unknown; &"
    );
}

/// A letter with an accent right after a tag closed the IDE: the text was cut inside it.
#[test]
fn text_after_a_tag_may_start_with_any_character() {
    let lines = layout("<p align=\"center\">Éditeur de code</p><div>été</div>日本");
    let texts: Vec<&str> = lines
        .iter()
        .filter_map(|line| match line {
            Line::Content { items, .. } => match &items[0] {
                Inline::Text { text, .. } => Some(text.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["Éditeur de code", "été", "日本"]);
}

/// Whatever is written, broken tags included, is read without a panic.
#[test]
fn any_html_is_read() {
    let pieces = [
        "<",
        ">",
        "</",
        "/>",
        "<!--",
        "-->",
        "=",
        "\"",
        "'",
        "&",
        ";",
        "&#",
        "&#x",
        " ",
        "\n",
        "é",
        "日",
        "🙂",
        "a",
        "p",
        "img",
        "src",
        "href",
        "<a href=\"é",
        "<b>",
        "</b>",
        "<br>",
        "<img src='",
        "<li>",
        "<ol start=",
        "<h1>",
        "&amp;",
        "&é;",
        "<é>",
        "<p é=é>",
        "\u{a0}",
    ];
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    for _ in 0..20_000 {
        let mut html = String::new();
        for _ in 0..12 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            html.push_str(pieces[(seed >> 33) as usize % pieces.len()]);
        }
        layout(&html);
        if let Some(converted) = inline_to_markdown(&html) {
            for offset in 0..=converted.text.len() {
                converted.source_offset(offset);
            }
        }
    }
}

/// A table keeps its rows and its cells: they are drawn in columns.
#[test]
fn a_table_keeps_its_rows_and_cells() {
    let lines = layout(
        "<p>avant</p><table>
 <tr><th>Nom</th><th>Valeur</th></tr>
 <tr><td>Bêta <b>long</b></td><td><p>un</p><p>deux</p></td>
</table>après",
    );
    assert_eq!(lines.len(), 3);
    let Line::Table { rows, .. } = &lines[1] else {
        panic!("{lines:?}")
    };
    let texts = |cell: &Vec<Inline>| -> Vec<String> {
        cell.iter()
            .map(|piece| match piece {
                Inline::Text { text, strong, .. } if *strong => format!("*{text}*"),
                Inline::Text { text, .. } => text.clone(),
                Inline::Break => "/".into(),
                Inline::Image { src, .. } => src.clone(),
            })
            .collect()
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(texts(&rows[0][0]), ["*Nom*"]);
    assert_eq!(texts(&rows[0][1]), ["*Valeur*"]);
    assert_eq!(texts(&rows[1][0]), ["Bêta ", "*long*"]);
    // The paragraphs of a cell are its lines.
    assert_eq!(texts(&rows[1][1]), ["un", "/", "deux"]);
    assert!(matches!(
        &lines[2],
        Line::Content { items, .. } if matches!(&items[0], Inline::Text { text, .. } if text == "après")
    ));
    // Never closed: shown all the same.
    assert!(matches!(
        &layout("<table><tr><td>a")[..],
        [Line::Table { rows, .. }] if rows.len() == 1
    ));
}
