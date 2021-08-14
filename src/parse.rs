use anyhow::{Context, Result};
use regex::Regex;
use std::collections::BTreeMap;

#[derive(Debug)]
pub enum Note {
    Highlight(String),
    Comment(String),
}

pub fn myclippings(data: &str) -> Result<BTreeMap<String, Vec<Note>>> {
    let mut output: BTreeMap<String, Vec<Note>> = BTreeMap::new();
    for note in data.split("==========\r\n") {
        if let Some((title, tidied_note)) = parse_note(note) {
            let entry = output.entry(title).or_insert_with(Vec::new);
            entry.push(tidied_note);
        }
    }
    Ok(output)
}

pub fn webexport(data: &str) -> Result<BTreeMap<String, Vec<Note>>> {
    let re_title = Regex::new(r#"<h3.*>(.*)</h3>"#)
        .with_context(|| "Failed to create regex for webexport title")?;
    let title: String = re_title
        .captures_iter(&data)
        .take(1)
        .map(|x| x[1].to_string())
        .collect();
    let re_hi_or_note = Regex::new(r#"(?s)<span.*?id="(highlight|note)".*?>(.*?)</span>"#)
        .with_context(|| "Failed to create regex for webexport highlight/note")?;
    let mut output: BTreeMap<String, Vec<Note>> = BTreeMap::new();
    for cap in re_hi_or_note.captures_iter(&data) {
        let entry = output.entry(title.clone()).or_insert_with(Vec::new);
        let tidy_entry = cap[2].replace("\n", "");
        if !tidy_entry.is_empty() {
            match &cap[1] {
                "highlight" => entry.push(Note::Highlight(tidy_entry)),
                "note" => entry.push(Note::Comment(tidy_entry)),
                _ => unreachable!(),
            }
        }
    }
    Ok(output)
}

// a 'note' is anything between the '====' in a `My Clippings.txt`
pub fn parse_note(note: &str) -> Option<(String, Note)> {
    let mut lines = note.lines();
    let title = lines
        .next()
        .map(|x| x.trim().trim_start_matches('\u{feff}'));
    let is_highlight = lines
        .next()
        .filter(|x| x.starts_with("- Your Highlight"))
        .is_some();
    let note = lines
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<String>()
        .trim()
        .to_string();
    match title {
        None => None,
        Some(t) => {
            if note.is_empty() {
                None
            } else if is_highlight {
                Some((t.to_string(), Note::Highlight(note)))
            } else {
                Some((t.to_string(), Note::Comment(note)))
            }
        }
    }
}
