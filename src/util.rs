use anyhow::Result;
use skim::prelude::*;
use std::io::Cursor;

pub fn choose_from_list(ls: &[String], filter: Option<String>) -> Result<Vec<String>> {
    let options = SkimOptionsBuilder::default().multi(true).build().unwrap();

    let inputstr = if let Some(query) = filter {
        ls
            .iter()
            .filter(|title| title.to_lowercase().contains(&query))
            .map(|title| title.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        ls.join("\n")
    };
    // `SkimItemReader` is a helper to turn any `BufRead` into a stream of `SkimItem`
    // `SkimItem` was implemented for `AsRef<str>` by default
    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Cursor::new(inputstr));

    // `run_with` would read and show items from the stream
    let selected_books = Skim::run_with(&options, Some(items))
        .map(|out| out.selected_items)
        .unwrap_or_default();

    let titles = selected_books
        .iter()
        .map(|x| x.output().to_string())
        .collect();
    Ok(titles)
}
