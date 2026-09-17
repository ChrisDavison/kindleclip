use crate::model::{Book, Clipping, Kind};
use crate::state::BookState;
use anyhow::Result;

pub struct ExportOpts {
    pub as_list: bool,
    pub only_marked: bool,
}

/// Render a book in the kindleclip markdown format:
/// `# Title` then `## Notes` (or one `##` section per cluster plus
/// `## Unclustered`), with one paragraph per clipping. Ignored clippings
/// never export. With no clusters, no annotations and nothing ignored the
/// output is byte-identical to the pre-web CLI export.
pub fn render_book(book: &Book, book_state: &BookState, opts: &ExportOpts) -> String {
    let exported: Vec<&Clipping> = book
        .clippings
        .iter()
        .filter(|c| !book_state.is_ignored(&c.id))
        .filter(|c| !opts.only_marked || book_state.is_marked(&c.id))
        .collect();

    let mut sections: Vec<(String, Vec<&Clipping>)> = Vec::new();
    if opts.only_marked || book_state.clusters.is_empty() {
        sections.push(("Notes".to_string(), exported));
    } else {
        for cluster in &book_state.clusters {
            let items: Vec<&Clipping> = cluster
                .clipping_ids
                .iter()
                .filter_map(|id| exported.iter().find(|c| c.id == *id).copied())
                .collect();
            sections.push((cluster.name.clone(), items));
        }
        let unclustered: Vec<&Clipping> = exported
            .iter()
            .filter(|c| !book_state.clusters.iter().any(|cl| cl.clipping_ids.contains(&c.id)))
            .copied()
            .collect();
        sections.push(("Unclustered".to_string(), unclustered));
    }
    sections.retain(|(_, items)| !items.is_empty());

    let (joiner, start) = if opts.as_list { ("\n", "- ") } else { ("\n\n", "") };
    let mut out = format!("# {}\n\n", book.title);
    for (i, (name, items)) in sections.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        out.push_str(&format!("## {name}\n\n"));
        let rendered: Vec<String> = items
            .iter()
            .map(|c| render_item(c, book_state, start, opts.as_list))
            .collect();
        out.push_str(&rendered.join(joiner));
    }
    out
}

/// True when a book has at least one clipping that would export.
pub fn has_exportable(book: &Book, book_state: &BookState, opts: &ExportOpts) -> bool {
    book.clippings.iter().any(|c| {
        !book_state.is_ignored(&c.id) && (!opts.only_marked || book_state.is_marked(&c.id))
    })
}

fn render_item(c: &Clipping, book_state: &BookState, start: &str, as_list: bool) -> String {
    let clean = |s: &str| s.replace('\r', "");
    let mut body = match c.kind {
        Kind::Highlight => clean(&c.text),
        Kind::Note => format!("NOTE: {}", clean(&c.text)),
    };
    if let Some(note) = &c.note {
        if !note.trim().is_empty() {
            let line = format!("NOTE: {}", clean(note));
            if as_list {
                body.push_str(&format!("\n  {line}"));
            } else {
                body.push_str(&format!("\n\n{line}"));
            }
        }
    }
    if !as_list {
        if let Some(cs) = book_state.clipping_state(&c.id) {
            if !cs.annotation.trim().is_empty() {
                let quoted = clean(&cs.annotation).replace('\n', "\n> ");
                body.push_str(&format!("\n\n> {quoted}"));
            }
        }
    }
    format!("{start}{body}")
}

/// Build an in-memory zip from named text entries.
pub fn render_zip(entries: Vec<(String, String)>) -> Result<Vec<u8>> {
    let buf = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(buf);
    let options = zip::write::FileOptions::default();
    for (name, body) in entries {
        writer.start_file(name, options.clone())?;
        std::io::Write::write_all(&mut writer, body.as_bytes())?;
    }
    let cursor = writer.finish()?;
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Source;
    use crate::state::{ClusterState, ClippingState};

    fn clip(id: &str, kind: Kind, text: &str) -> Clipping {
        Clipping {
            id: id.to_string(),
            kind,
            text: text.to_string(),
            note: None,
            location: None,
            page: None,
            chapter: None,
            added: None,
            source: Source::MyClippings,
        }
    }

    fn book(clippings: Vec<Clipping>) -> Book {
        Book {
            id: "b".into(),
            title: "Test Book".into(),
            author: None,
            clippings,
            last_pos: 1,
        }
    }

    #[test]
    fn no_clusters_matches_legacy_format() {
        let b = book(vec![
            clip("c1", Kind::Highlight, "first"),
            clip("c2", Kind::Note, "second"),
        ]);
        let out = render_book(&b, &BookState::default(), &ExportOpts { as_list: false, only_marked: false });
        assert_eq!(out, "# Test Book\n\n## Notes\n\nfirst\n\nNOTE: second");
    }

    #[test]
    fn list_mode_matches_legacy_format() {
        let b = book(vec![
            clip("c1", Kind::Highlight, "first"),
            clip("c2", Kind::Note, "second"),
        ]);
        let out = render_book(&b, &BookState::default(), &ExportOpts { as_list: true, only_marked: false });
        assert_eq!(out, "# Test Book\n\n## Notes\n\n- first\n- NOTE: second");
    }

    #[test]
    fn clusters_render_as_sections_then_unclustered() {
        let b = book(vec![clip("c1", Kind::Highlight, "first"), clip("c2", Kind::Highlight, "second")]);
        let mut bs = BookState::default();
        bs.clusters.push(ClusterState {
            id: "cl1".into(),
            name: "Favourites".into(),
            clipping_ids: vec!["c2".into()],
        });
        bs.clusters.push(ClusterState {
            id: "cl2".into(),
            name: "Empty".into(),
            clipping_ids: vec![],
        });
        let out = render_book(&b, &bs, &ExportOpts { as_list: false, only_marked: false });
        assert_eq!(
            out,
            "# Test Book\n\n## Favourites\n\nsecond\n\n## Unclustered\n\nfirst",
            "empty clusters are skipped"
        );
    }

    #[test]
    fn annotation_exports_as_blockquote() {
        let b = book(vec![clip("c1", Kind::Highlight, "first")]);
        let mut bs = BookState::default();
        bs.clippings.insert(
            "c1".into(),
            ClippingState { marked: false, ignored: false, annotation: "my thought".into() },
        );
        let out = render_book(&b, &bs, &ExportOpts { as_list: false, only_marked: false });
        assert_eq!(out, "# Test Book\n\n## Notes\n\nfirst\n\n> my thought");
    }

    #[test]
    fn attached_note_exports_after_highlight() {
        let mut c = clip("c1", Kind::Highlight, "first");
        c.note = Some("the author's note".to_string());
        let b = book(vec![c]);
        let out = render_book(&b, &BookState::default(), &ExportOpts { as_list: false, only_marked: false });
        assert_eq!(out, "# Test Book\n\n## Notes\n\nfirst\n\nNOTE: the author's note");
    }

    #[test]
    fn ignored_clippings_never_export() {
        let b = book(vec![clip("c1", Kind::Highlight, "first"), clip("c2", Kind::Highlight, "second")]);
        let mut bs = BookState::default();
        bs.clipping("c1").ignored = true;
        bs.clipping("c1").marked = true;
        let out = render_book(&b, &bs, &ExportOpts { as_list: false, only_marked: false });
        assert_eq!(out, "# Test Book\n\n## Notes\n\nsecond");
    }

    #[test]
    fn marked_only_is_flat_in_file_order() {
        let b = book(vec![clip("c1", Kind::Highlight, "first"), clip("c2", Kind::Highlight, "second")]);
        let mut bs = BookState::default();
        bs.clipping("c2").marked = true;
        bs.clusters.push(ClusterState {
            id: "cl1".into(),
            name: "Favourites".into(),
            clipping_ids: vec!["c2".into()],
        });
        let out = render_book(&b, &bs, &ExportOpts { as_list: false, only_marked: true });
        assert_eq!(out, "# Test Book\n\n## Notes\n\nsecond");
    }

    #[test]
    fn renders_zip_with_one_entry_per_book() {
        let b = book(vec![clip("c1", Kind::Highlight, "body")]);
        let body = render_book(&b, &BookState::default(), &ExportOpts { as_list: false, only_marked: false });
        let bytes = render_zip(vec![(format!("{}.md", b.filestem()), body)]).unwrap();
        let reader = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(reader.file_names().collect::<Vec<_>>(), vec!["test-book.md"]);
    }
}
