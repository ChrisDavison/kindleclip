use crate::model::*;
use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use std::collections::HashMap;

/// Parse a Kindle 'My Clippings.txt' into books with their clippings in
/// file order. Entries that fail to parse are skipped with a message,
/// matching the previous CLI behaviour.
pub fn parse(data: &str) -> Result<Vec<Book>> {
    let normalized = data.replace("\r\n", "\n");
    let mut map: HashMap<String, Book> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for (i, note) in normalized.split("==========\n").enumerate() {
        match parse_note(note) {
            Ok((title, mut clipping)) => {
                let key = merge_key(&title);
                let book = map.entry(key.clone()).or_insert_with(|| {
                    order.push(key.clone());
                    Book {
                        id: fnv1a64(&key),
                        title: title.clone(),
                        author: None,
                        clippings: Vec::new(),
                        last_pos: i,
                    }
                });
                clipping.id = clipping_id(&key, &clipping.text);
                clipping.source = Source::MyClippings;
                book.clippings.push(clipping);
                book.last_pos = i;
            }
            Err(e) => eprintln!("PARSE FAIL: {e}\n{note}"),
        }
    }
    let books = order.into_iter().map(|k| map.remove(&k).unwrap()).collect();
    Ok(books)
}

fn parse_note(note: &str) -> Result<(String, Clipping)> {
    if note.is_empty() {
        return Err(anyhow!("Empty"));
    }
    let mut lines = note.lines();
    let title = lines
        .next()
        .ok_or_else(|| anyhow!("No title"))?
        .trim()
        .trim_start_matches('\u{feff}')
        .to_string();
    if title.is_empty() {
        return Err(anyhow!("No title"));
    }

    let metadata_line = lines.next().ok_or_else(|| anyhow!("No metadata line"))?;
    let idx_page = metadata_line.find("on page ").map(|x| (x + 8, "page"));
    let idx_location = metadata_line
        .find("at location ")
        .map(|x| (x + 12, "location"));
    let (i1, loc_kind) = match (idx_page, idx_location) {
        (Some(p), _) => p,
        (None, Some(l)) => l,
        _ => return Err(anyhow!("No page or location")),
    };
    let i2 = metadata_line
        .find('|')
        .ok_or_else(|| anyhow!("No separation between page and date"))?;
    if i2 <= i1 {
        return Err(anyhow!("Metadata line ends before its page or location"));
    }
    let segment = metadata_line[i1..i2 - 1].trim();
    let pages: Vec<&str> = segment.split('-').collect();

    let date_start = metadata_line
        .find("Added on")
        .ok_or_else(|| anyhow!("No date"))?
        + 9;
    let added = metadata_line
        .get(date_start..)
        .and_then(parse_added);

    let is_highlight = metadata_line.starts_with("- Your Highlight");

    let text = lines
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<String>()
        .trim()
        .to_string();

    let location = Some(format!("{loc_kind} {segment}"));
    let page = pages.first().and_then(|p| p.trim().parse::<i64>().ok());

    Ok((
        title,
        Clipping {
            id: String::new(),
            kind: if is_highlight {
                Kind::Highlight
            } else {
                Kind::Note
            },
            text,
            note: None,
            location,
            page,
            chapter: None,
            added,
            source: Source::MyClippings,
        },
    ))
}

fn parse_added(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    // e.g. "Monday, May 12, 2025 3:04:12 PM"
    for fmt in ["%A, %B %d, %Y %I:%M:%S %p", "%A, %B %d, %Y %l:%M:%S %p"] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(dt.and_utc());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "The Snowball (Alice Schroeder)\r\n\
        - Your Highlight on page 123-124 | Added on Monday, May 12, 2025 3:04:12 PM\r\n\
        \r\n\
        Life is like a snowball.\r\n\
        ==========\r\n\
        The Snowball (Alice Schroeder)\r\n\
        - Your Note on page 123 | Added on Tuesday, May 13, 2025 9:05:00 AM\r\n\
        \r\n\
        worth remembering\r\n\
        ==========\r\n\
        Another Book\r\n\
        - Your Highlight at location 1234-1240 | Added on Wednesday, May 14, 2025 11:00:00 AM\r\n\
        \r\n\
        Location only entry.\r\n\
        ==========\r\n";

    #[test]
    fn parses_highlights_notes_and_locations() {
        let books = parse(SAMPLE).unwrap();
        assert_eq!(books.len(), 2);

        let snowball = &books[0];
        assert_eq!(snowball.title, "The Snowball (Alice Schroeder)");
        assert_eq!(snowball.clippings.len(), 2);

        let first = &snowball.clippings[0];
        assert_eq!(first.kind, Kind::Highlight);
        assert_eq!(first.text, "Life is like a snowball.");
        assert_eq!(first.page, Some(123));
        assert_eq!(first.location.as_deref(), Some("page 123-124"));
        assert!(first.added.is_some(), "date should parse");

        let second = &snowball.clippings[1];
        assert_eq!(second.kind, Kind::Note);
        assert_eq!(second.text, "worth remembering");

        let other = &books[1];
        let only = &other.clippings[0];
        assert_eq!(only.location.as_deref(), Some("location 1234-1240"));
        assert_eq!(only.page, Some(1234));
    }

    #[test]
    fn parses_lf_line_endings() {
        let lf = SAMPLE.replace("\r\n", "\n");
        let books = parse(&lf).unwrap();
        assert_eq!(books.len(), 2);
        assert_eq!(books[0].clippings.len(), 2);
    }

    #[test]
    fn skips_malformed_entries() {
        let data = "Good Book\r\n- Your Highlight on page 1 | Added on Monday, May 12, 2025 3:04:12 PM\r\n\r\nfine\r\n==========\r\nBad Entry\r\nno metadata here\r\n==========\r\n";
        let books = parse(data).unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].clippings.len(), 1);
    }

    #[test]
    fn identical_text_yields_identical_ids() {
        let data = "Book\r\n- Your Highlight on page 1 | Added on Monday, May 12, 2025 3:04:12 PM\r\n\r\nsame text\r\n==========\r\nBook\r\n- Your Highlight on page 2 | Added on Tuesday, May 13, 2025 3:04:12 PM\r\n\r\nsame text\r\n==========\r\n";
        let books = parse(data).unwrap();
        let ids: Vec<&str> = books[0].clippings.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids.len(), 2, "parse keeps both; serve-mode merge dedupes");
        assert_eq!(ids[0], ids[1], "same title and text must map to one id");
    }
}
