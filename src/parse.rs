use anyhow::{Context, Result};
use regex::Regex;
use std::collections::BTreeMap;

pub fn myclippings(data: &str) -> Result<BTreeMap<String, Vec<String>>> {
    let mut output: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for note in data.split("==========\r\n") {
        if let Some((title, tidied_note)) = parse_note(note) {
            let entry = output.entry(title).or_insert_with(Vec::new);
            entry.push(tidied_note);
        }
    }
    Ok(output)
}

pub fn webexport(data: &str) -> Result<BTreeMap<String, Vec<String>>> {
    let re_title = Regex::new(r#"<h3.*>(.*)</h3>"#)
        .with_context(|| "Failed to create regex for webexport title")?;
    let title: String = re_title
        .captures_iter(&data)
        .take(1)
        .map(|x| x[1].to_string())
        .collect();
    let re_hi_or_note = Regex::new(r#"(?s)<span.*?id="(?:highlight|note)".*?>(.*?)</span>"#)
        .with_context(|| "Failed to create regex for webexport highlight/note")?;
    let mut output: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for cap in re_hi_or_note.captures_iter(&data) {
        let entry = output.entry(title.clone()).or_insert_with(Vec::new);
        entry.push(cap[1].replace("\n", ""));
    }
    Ok(output)
}

pub fn parse_note(note: &str) -> Option<(String, String)> {
    let mut lines = note.lines();
    let title = lines
        .next()
        .map(|x| x.trim().trim_start_matches('\u{feff}'))
        .unwrap_or("");
    let tidied_note = lines.map(tidy_note_line).collect();
    if title.is_empty() {
        None
    } else {
        Some((title.to_string(), tidied_note))
    }
}

fn tidy_note_line(line: &str) -> String {
    if line.starts_with("- Your Highlight") {
        "".to_string()
    } else if line.starts_with("- Your Note") {
        "NOTE FOR PREVIOUS HIGHLIGHT: ".to_string()
    } else {
        format!("{}\n", line)
    }
}
