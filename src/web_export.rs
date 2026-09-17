use crate::model::*;
use anyhow::{Context, Result};
use regex::Regex;

fn get_h3_title(data: &str) -> Result<&str> {
    let re_title = Regex::new(r#"<h3.*>(.*)</h3>"#)
        .with_context(|| "Failed to create regex for webexport title")?;
    let title = match re_title.find(data) {
        None => "",
        Some(mat) => {
            let right_angle = data[mat.start()..].find('>').unwrap() + 1 + mat.start();
            let left_angle = data[mat.start() + 1..].find('<').unwrap() + 1 + mat.start();
            &data[right_angle..left_angle]
        }
    };
    Ok(title)
}

/// Parse a saved Kindle library web page. The page carries no dates, so
/// recency falls back to file position.
pub fn parse(data: &str) -> Result<Vec<Book>> {
    let title = get_h3_title(data)?;
    let re_hi_or_note = Regex::new(r#"(?s)<span.*?id="(highlight|note)".*?>(.*?)</span>"#)
        .with_context(|| "Failed to create regex for webexport highlight/note")?;
    let key = merge_key(title);
    let mut clippings = Vec::new();
    let mut last_pos = 0;
    for (i, cap) in re_hi_or_note.captures_iter(data).enumerate() {
        last_pos = i;
        let tidy_entry = cap[2].replace(['\r', '\n'], " ");
        if !tidy_entry.is_empty() {
            let kind = match &cap[1] {
                "highlight" => Kind::Highlight,
                "note" => Kind::Note,
                _ => unreachable!(),
            };
            clippings.push(Clipping {
                id: clipping_id(&key, &tidy_entry),
                kind,
                text: tidy_entry,
                note: None,
                location: None,
                page: None,
                chapter: None,
                added: None,
                source: Source::WebPage,
            });
        }
    }
    Ok(vec![Book {
        id: fnv1a64(&key),
        title: title.to_string(),
        author: None,
        clippings,
        last_pos,
    }])
}

#[test]
fn find_title_test() {
    let tmp = "<h3 blah>this is the title</h3>";
    assert_eq!("this is the title", get_h3_title(tmp).unwrap());
}

#[test]
fn parses_spans_into_clippings() {
    let data = "<h3>My Book</h3>\
        <span id=\"highlight\">first highlight</span>\
        <span id=\"note\">a note</span>";
    let books = parse(data).unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].title, "My Book");
    assert_eq!(books[0].clippings.len(), 2);
    assert_eq!(books[0].clippings[0].kind, Kind::Highlight);
    assert_eq!(books[0].clippings[1].kind, Kind::Note);
    assert_eq!(books[0].clippings[0].source, Source::WebPage);
}
