use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// FNV-1a 64-bit hash as hex. Unlike std's DefaultHasher this is stable
/// across runs and compiler versions, so it can key persisted state.
pub fn fnv1a64(data: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Titles merge on a normalised key: a trailing parenthesised group is
/// stripped, so a Kindle title like "Title (Author)" matches the clean
/// title of a KOReader export of the same book.
pub fn merge_key(title: &str) -> String {
    let t = title.trim();
    let t = if t.ends_with(')') {
        match t.rfind('(') {
            Some(i) => t[..i].trim_end(),
            None => t,
        }
    } else {
        t
    };
    t.to_string()
}

/// A clipping's identity: book merge key plus the clipping text. Date and
/// page are deliberately excluded so the same highlight from Kindle-native
/// and KOReader sources collapses to one entry.
pub fn clipping_id(book_title_key: &str, text: &str) -> String {
    fnv1a64(&format!("{book_title_key}\u{1}{text}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    MyClippings,
    WebPage,
    KoReader,
}

impl Source {
    pub fn label(&self) -> &'static str {
        match self {
            Source::MyClippings => "kindle",
            Source::WebPage => "web",
            Source::KoReader => "koreader",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Highlight,
    Note,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clipping {
    pub id: String,
    pub kind: Kind,
    pub text: String,
    /// Note attached by KOReader to its parent highlight.
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub chapter: Option<String>,
    #[serde(default)]
    pub added: Option<DateTime<Utc>>,
    pub source: Source,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    pub clippings: Vec<Clipping>,
    /// Position of the book's last entry in its source file. Used as a
    /// recency fallback when the source carries no dates.
    pub last_pos: usize,
}

impl Book {
    pub fn filestem(&self) -> String {
        let bad_chars = ['(', ')', ',', ':'];
        let letter_tidier = |letter| {
            if bad_chars.contains(&letter) {
                "".to_string()
            } else if letter == ' ' {
                "-".to_string()
            } else {
                letter.to_lowercase().to_string()
            }
        };
        self.title.chars().map(letter_tidier).collect()
    }

    /// Most recent clipping date, if any source carried dates.
    pub fn latest(&self) -> Option<DateTime<Utc>> {
        self.clippings.iter().filter_map(|c| c.added).max()
    }

    pub fn source_labels(&self) -> String {
        let mut labels: Vec<&str> = Vec::new();
        for c in &self.clippings {
            let l = c.source.label();
            if !labels.contains(&l) {
                labels.push(l);
            }
        }
        labels.join(", ")
    }
}

/// Parse a source file by its extension.
pub fn parse_source(path: &Path, data: &str) -> Result<Vec<Book>> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("txt") => crate::my_clippings::parse(data),
        Some("html") | Some("htm") => crate::web_export::parse(data),
        Some("json") => crate::koreader_json::parse(data),
        Some("lua") => crate::koreader_lua::parse(data),
        other => Err(anyhow!(
            "Unsupported file format {:?} ({}): want .txt (My Clippings.txt), \
             .html (saved Kindle library page) .json (KOReader export) or .lua (KOReader metadata)",
            other.unwrap_or(""),
            path.display()
        )),
    }
}

/// Merge parsed books into the library, grouping by merge key, preferring a
/// clean title (no trailing author parenthesis) and the first author found,
/// and deduplicating identical clippings.
pub fn merge_into(library: &mut HashMap<String, Book>, books: Vec<Book>) {
    for book in books {
        let key = merge_key(&book.title);
        let id = fnv1a64(&key);
        let entry = library.entry(id.clone()).or_insert_with(|| Book {
            id: id.clone(),
            title: book.title.clone(),
            author: book.author.clone(),
            clippings: Vec::new(),
            last_pos: 0,
        });
        // Prefer the title without an embedded "(Author)" suffix.
        if entry.title != key && book.title == key {
            entry.title = book.title.clone();
        }
        if entry.author.is_none() {
            entry.author = book.author.clone();
        }
        entry.last_pos = entry.last_pos.max(book.last_pos);
        for c in book.clippings {
            if !entry.clippings.iter().any(|x| x.id == c.id) {
                entry.clippings.push(c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a64_is_stable() {
        assert_eq!(fnv1a64(""), "cbf29ce484222325");
        assert_eq!(fnv1a64("a"), "af63dc4c8601ec8c");
        assert_eq!(fnv1a64("a"), fnv1a64("a"));
    }

    #[test]
    fn merge_key_strips_trailing_parenthetical() {
        assert_eq!(merge_key("The Snowball (Alice Schroeder)"), "The Snowball");
        assert_eq!(merge_key("  Plain Title  "), "Plain Title");
        assert_eq!(merge_key("A (B) C"), "A (B) C");
        assert_eq!(merge_key("Ends in ("), "Ends in (");
    }

    fn sample_book(title: &str, texts: &[&str], source: Source) -> Book {
        let key = merge_key(title);
        Book {
            id: fnv1a64(&key),
            title: title.to_string(),
            author: None,
            clippings: texts
                .iter()
                .map(|t| Clipping {
                    id: clipping_id(&key, t),
                    kind: Kind::Highlight,
                    text: t.to_string(),
                    note: None,
                    location: None,
                    page: None,
                    chapter: None,
                    added: None,
                    source,
                })
                .collect(),
            last_pos: texts.len(),
        }
    }

    #[test]
    fn merge_groups_by_key_and_dedupes() {
        let mut library = HashMap::new();
        merge_into(
            &mut library,
            vec![sample_book("The Book (An Author)", &["a", "b"], Source::MyClippings)],
        );
        merge_into(
            &mut library,
            vec![sample_book("The Book", &["b", "c"], Source::KoReader)],
        );
        assert_eq!(library.len(), 1);
        let book = library.values().next().unwrap();
        assert_eq!(book.title, "The Book");
        assert_eq!(book.clippings.len(), 3, "duplicate 'b' collapsed");
        let sources: Vec<&str> = book.clippings.iter().map(|c| c.source.label()).collect();
        assert!(sources.contains(&"kindle") && sources.contains(&"koreader"));
    }
}
