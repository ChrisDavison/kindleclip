use crate::note::*;
use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;

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

pub fn parse(data: &str) -> Result<BooknoteMap> {
    let title = get_h3_title(data)?;
    let re_hi_or_note = Regex::new(r#"(?s)<span.*?id="(highlight|note)".*?>(.*?)</span>"#)
        .with_context(|| "Failed to create regex for webexport highlight/note")?;
    let mut output: BooknoteMap = HashMap::new();
    for (i, cap) in re_hi_or_note.captures_iter(data).enumerate() {
        let entry = output.entry(title.to_string()).or_default();
        let tidy_entry = cap[2].replace(['\r', '\n'], " ");
        if !tidy_entry.is_empty() {
            let highlight_type = match &cap[1] {
                "highlight" => HighlightType::Highlight,
                "note" => HighlightType::Comment,
                _ => unreachable!(),
            };
            entry.title = title.to_string();
            entry.mru_indice = i;
            entry.highlights.push(Highlight {
                highlight_type,
                pages: ["", ""],
                date_added: "",
                highlight: tidy_entry,
            })
        }
    }
    Ok(output)
}

#[test]
fn find_title_test() {
    let tmp = "<h3 blah>this is the title</h3>";
    assert_eq!("this is the title", get_h3_title(tmp).unwrap());
}
