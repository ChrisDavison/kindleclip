use crate::note::*;
use anyhow::{anyhow, Result};
use std::collections::HashMap;


pub fn parse(data: &str) -> Result<(BookNotes, Vec<String>)> {
    let mut output: HashMap<String, Vec<Highlight>> = HashMap::new();
    let mut titles_last_seen_indice: HashMap<String, usize> = HashMap::new();
    for (i, note) in data.split("==========\r\n").enumerate() {
        if let Ok(highlight) = parse_note(note) {
            let to_insert = match titles_last_seen_indice.remove(highlight.name) {
                Some(c) => c,
                None => i,
            };
            titles_last_seen_indice.insert(highlight.name.to_string(), to_insert);
            let entry = output
                .entry(highlight.name.to_string())
                .or_default();
            entry.push(highlight);
        }
    }
    let mut ordered_titles: Vec<(usize, String)> = titles_last_seen_indice
        .iter()
        .map(|(k, v)| (*v, k.clone()))
        .collect();
    ordered_titles.sort();
    let only_ordered_titles: Vec<String> = ordered_titles.iter().map(|(_, k)| k.clone()).collect();
    Ok((output, only_ordered_titles))
}

fn parse_note(note: &str) -> Result<Highlight> {
    if note.is_empty() {
        return Err(anyhow!("Empty"));
    }
    let mut lines = note.lines();
    let title = lines
        .next()
        .map(|x| x.trim().trim_start_matches('\u{feff}'))
        .expect(note);

    let metadata_line = lines.next().expect("No metadata line");
    let idx_page = metadata_line.find("on page ").map(|x| x + 8);
    let idx_location = metadata_line.find("at location ").map(|x| x + 12);
    let i1 = idx_page.or(idx_location).expect("No page or location");
    let i2 = metadata_line
        .find('|')
        .expect("No separation between page and date");
    let pages: Vec<_> = metadata_line[i1..i2 - 1].split('-').collect();

    let date_start = metadata_line.find("Added on").expect("No date") + 9;
    let added_on = &metadata_line[date_start..];

    let is_highlight = metadata_line.starts_with("- Your Highlight");

    let note = lines
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<String>()
        .trim()
        .to_string();

    if pages.is_empty() {
        println!("`{}`", title);
        println!("`{}`", metadata_line);
    }

    Ok(Highlight {
        name: title,
        highlight_type: if is_highlight {
            HighlightType::Highlight
        } else {
            HighlightType::Comment
        },
        pages: [pages[0], if pages.len() > 1 { pages[1] } else { pages[0] }],
        date_added: added_on,
        highlight: note,
    })
}
