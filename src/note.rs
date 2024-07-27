use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

pub type BooknoteMap<'a> = HashMap<String, BookNotes<'a>>;

#[derive(Default)]
pub struct BookNotes<'a> {
    pub title: String,
    pub highlights: Vec<Highlight<'a>>,
    pub mru_indice: usize,
}

impl<'a> BookNotes<'a> {
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

    pub fn export(&self, outdir: &PathBuf, as_list: bool) -> Result<()> {
        let (joiner, start) = if as_list { ("\n", "- ") } else { ("\n\n", "") };
        let filestem = self.filestem();
        let notes = self
            .highlights
            .iter()
            .map(|n| format!("{}{}", start, n))
            .collect::<Vec<String>>()
            .join(joiner);
        let mut output_filename: PathBuf = outdir.into();

        output_filename.push(filestem + ".md");
        let notes = format!("# {}\n\n## Notes\n\n{}", self.title, notes);
        std::fs::write(&output_filename, notes)
            .with_context(|| format!("Failed to write file {:?}", output_filename))
    }
}

impl<'a> std::fmt::Display for BookNotes<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "{} ({} highlights)", self.title, self.highlights.len())
    }
}

#[derive(Debug)]
pub struct Highlight<'a> {
    pub highlight_type: HighlightType,
    #[allow(dead_code)]
    pub pages: [&'a str; 2],
    #[allow(dead_code)]
    pub date_added: &'a str,
    pub highlight: String,
}

#[derive(Debug)]
pub enum HighlightType {
    Highlight,
    Comment,
}

impl<'a> std::fmt::Display for Highlight<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let prefix = match self.highlight_type {
            HighlightType::Highlight => "",
            HighlightType::Comment => "NOTE: ",
        };
        write!(f, "{}{}", prefix, self.highlight.replace('\r', ""))
    }
}
