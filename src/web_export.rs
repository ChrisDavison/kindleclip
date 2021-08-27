use crate::note::*;
use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;

pub fn parse(data: &str) -> Result<HashMap<String, Vec<Highlight>>> {
    let re_title = Regex::new(r#"<h3.*>(.*)</h3>"#)
        .with_context(|| "Failed to create regex for webexport title")?;
    let title: String = re_title
        .captures_iter(data)
        .take(1)
        .map(|x| x[1].to_string())
        .collect();
    let re_hi_or_note = Regex::new(r#"(?s)<span.*?id="(highlight|note)".*?>(.*?)</span>"#)
        .with_context(|| "Failed to create regex for webexport highlight/note")?;
    let mut output: HashMap<String, Vec<Highlight>> = HashMap::new();
    for cap in re_hi_or_note.captures_iter(data) {
        let entry = output.entry(title.clone()).or_insert_with(Vec::new);
        let tidy_entry = cap[2].replace("\r", "").replace("\n", "");
        if !tidy_entry.is_empty() {
            let highlight_type = match &cap[1] {
                "highlight" => HighlightType::Highlight,
                "note" => HighlightType::Comment,
                _ => unreachable!(),
            };
            entry.push(Highlight {
                name: title.clone(),
                highlight_type,
                pages: [String::new(), String::new()],
                date_added: String::new(),
                highlight: tidy_entry,
            })
        }
    }
    Ok(output)
}
