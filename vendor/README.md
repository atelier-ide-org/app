# vendor

Crates of crates.io kept here with a change of ours, and put in place of the published one by
`[patch.crates-io]` in the `Cargo.toml` of the IDE.

| Crate | Version | Change |
| --- | --- | --- |
| `egui_commonmark_backend` | 0.26.0 | The syntaxes and themes of the code blocks are loaded once and shared by every `CommonMarkCache` (`shared_syntaxes` in `src/misc.rs`): each cache compiled the regexes of a language again. Bold text and titles of the Markdown preview take the font of the text style named `strong` (`Style::to_richtext` in `src/misc.rs`): upstream only changes their color. The markers of the list items are drawn at the middle of their text, and a number is followed by a space (`src/elements.rs`). |
| `egui_commonmark` | 0.26.0 | Tables: the cells have room on their left and right, the header has a background and bold text (`table` in `src/parsers/pulldown.rs`). The blank lines after the first one between two blocks are kept (`extra_blank_lines`). |

When the version of `egui_commonmark` changes, copy the new `egui_commonmark` and `egui_commonmark_backend` here and
make the change again (look for `Atelier:` in the sources).
