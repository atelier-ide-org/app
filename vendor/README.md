# vendor

Crates of crates.io kept here with a change of ours, and put in place of the published one by
`[patch.crates-io]` in the `Cargo.toml` of the IDE.

| Crate | Version | Change |
| --- | --- | --- |
| `egui_commonmark_backend` | 0.26.0 | Bold text and titles of the Markdown preview take the font of the text style named `strong` (`Style::to_richtext` in `src/misc.rs`): upstream only changes their color. |

When the version of `egui_commonmark` changes, copy the new `egui_commonmark_backend` here and
make the change again (look for `Atelier:` in the sources).
