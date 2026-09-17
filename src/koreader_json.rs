use crate::model::*;
use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;

#[derive(Deserialize)]
struct ExportFile {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    entries: Vec<Entry>,
    #[serde(default)]
    documents: Vec<BookDoc>,
}

/// One book inside a multi-book KOReader export.
#[derive(Deserialize)]
struct BookDoc {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    page: Option<serde_json::Value>,
    #[serde(default)]
    time: Option<i64>,
    #[serde(default)]
    chapter: Option<String>,
    #[serde(default)]
    sort: Option<String>,
}

/// Parse a KOReader JSON highlight export, both the single-book shape
/// (`{title, author, entries}`) and the library shape (`{documents: [...]}`).
pub fn parse(data: &str) -> Result<Vec<Book>> {
    let file: ExportFile =
        serde_json::from_str(data).with_context(|| "Invalid KOReader JSON export")?;

    let docs: Vec<(String, Option<String>, Vec<Entry>)> = if !file.documents.is_empty() {
        file.documents
            .into_iter()
            .filter_map(|d| d.title.map(|t| (t, d.author, d.entries)))
            .collect()
    } else if let Some(title) = file.title {
        vec![(title, file.author, file.entries)]
    } else {
        return Err(anyhow::anyhow!(
            "KOReader JSON has neither 'documents' nor a 'title'"
        ));
    };

    let mut books = Vec::new();
    for (title, author, entries) in docs {
        let key = merge_key(&title);
        let mut clippings = Vec::new();
        let mut last_pos = 0;
        for (i, entry) in entries.into_iter().enumerate() {
            last_pos = i;
            let text = entry.text.unwrap_or_default();
            let note = entry.note.filter(|n| !n.trim().is_empty());
            if text.trim().is_empty() && note.is_none() {
                continue;
            }
            let kind = match entry.sort.as_deref() {
                // KOReader marks bookmarks with sort "bookmark"; keep them
                // visible as notes, same treatment as Kindle bookmark lines.
                Some("highlight") => Kind::Highlight,
                _ => Kind::Note,
            };
            let page = entry.page.as_ref().and_then(page_to_i64);
            let location = page.map(|p| format!("page {p}"));
            let added = entry.time.and_then(epoch_to_datetime);
            clippings.push(Clipping {
                id: clipping_id(&key, &text),
                kind,
                text,
                note,
                location,
                page,
                chapter: entry.chapter,
                added,
                source: Source::KoReader,
            });
        }
        books.push(Book {
            id: fnv1a64(&key),
            title,
            author,
            clippings,
            last_pos,
        });
    }
    Ok(books)
}

fn page_to_i64(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn epoch_to_datetime(secs: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(secs, 0).single()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_book_export() {
        let data = r#"{
            "title": "A Book",
            "author": "An Author",
            "entries": [
                {"page": 12, "time": 1700000000, "text": "first", "chapter": "One", "sort": "highlight"},
                {"page": 20, "time": 1700000100, "text": "second", "note": "my note", "sort": "highlight"},
                {"text": "", "sort": "highlight"},
                {"page": "N/A", "text": "third", "sort": "bookmark"}
            ],
            "created_on": 1700000200,
            "version": "20230101"
        }"#;
        let books = parse(data).unwrap();
        assert_eq!(books.len(), 1);
        let book = &books[0];
        assert_eq!(book.title, "A Book");
        assert_eq!(book.author.as_deref(), Some("An Author"));
        assert_eq!(book.clippings.len(), 3, "empty text entry skipped");

        let first = &book.clippings[0];
        assert_eq!(first.kind, Kind::Highlight);
        assert_eq!(first.page, Some(12));
        assert_eq!(first.location.as_deref(), Some("page 12"));
        assert_eq!(first.chapter.as_deref(), Some("One"));
        assert!(first.added.is_some());

        let second = &book.clippings[1];
        assert_eq!(second.note.as_deref(), Some("my note"));

        let third = &book.clippings[2];
        assert_eq!(third.kind, Kind::Note, "bookmark treated as note");
        assert_eq!(third.page, None, "'N/A' page is not a number");
    }

    #[test]
    fn parses_multi_book_documents_export() {
        let data = r#"{
            "created_on": 1700000200,
            "version": "20230101",
            "documents": [
                {"title": "Book One", "author": "A", "entries": [{"text": "x", "sort": "highlight"}]},
                {"title": "Book Two (Someone)", "entries": [{"text": "y", "sort": "highlight"}]}
            ]
        }"#;
        let books = parse(data).unwrap();
        assert_eq!(books.len(), 2);
        assert_eq!(books[0].title, "Book One");
        assert_eq!(books[1].clippings[0].text, "y");
        assert!(books[1].author.is_none());
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse("not json").is_err());
    }
}
