use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use structopt::StructOpt;

/// Parse a kindle 'My Clippings.txt', or saved webpage.
#[derive(StructOpt, Debug)]
struct Opts {
    file: PathBuf,
    outdir: PathBuf,

    /// Prompt for which books' notes to export
    #[structopt(short, long)]
    select: bool,
}

fn main() -> Result<()> {
    let args = Opts::from_args();

    if !args.outdir.is_dir() {
        if let Err(e) = std::fs::create_dir(&args.outdir) {
            eprintln!("Failed to create output dir {:?}: {}", args.outdir, e);
            std::process::exit(2);
        }
    }
    dbg!(&args);

    let ext = PathBuf::from(&args.file)
        .extension()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let parser = match ext.as_ref() {
        "html" => parse::webexport,
        "txt" => parse::myclippings,
        _ => {
            println!("Unsupported file format.");
            println!("Want html saved from kindle library webpage,");
            println!("or 'My Clippings.txt' from kindle storage.");
            return Err(anyhow!("Invalid file format"));
        }
    };

    let data = std::fs::read_to_string(&args.file).expect("Failed to read file");
    if let Ok(clippings) = parser(&data) {
        let titles = if args.select {
            let keys: Vec<String> = clippings.keys().map(|x| x.to_string()).collect();
            choose_from_list(&keys)?
        } else {
            clippings.keys().map(|x| x.to_string()).collect()
        };
        for title in titles {
            export_book_notes(&title, &clippings[&title], args.outdir.clone())?;
        }
    }
    Ok(())
}

fn export_book_notes(book: &str, notes: &[String], outdir: PathBuf) -> Result<()> {
    let mut output_filename = outdir;
    output_filename.push(bookname_to_filename(&book) + ".md");
    let header_and_notes = format!("# {}\n\n## Notes\n\n{}", book, notes.join("\n"));
    std::fs::write(&output_filename, header_and_notes)
        .with_context(|| format!("Failed to write file {:?}", output_filename))
}

fn choose_from_list(ls: &[impl ToString]) -> Result<Vec<String>> {
    for (i, key) in ls.iter().enumerate() {
        println!("{}: {}", i, key.to_string());
    }
    let mut response = String::new();
    if let Err(e) = std::io::stdin().read_line(&mut response) {
        eprintln!("Failed to read response: {}", e);
    }
    let mut books = Vec::new();
    for choice in response.split(|c| " ,".contains(c)) {
        let choice: usize = choice.trim().parse()?;
        books.push(ls[choice].to_string());
    }
    Ok(books)
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

fn bookname_to_filename(bookname: impl ToString) -> String {
    let bad_chars = vec!['(', ')', ',', ':'];
    let letter_tidier = |letter| {
        if bad_chars.contains(&letter) {
            "".to_string()
        } else if letter == ' ' {
            "-".to_string()
        } else {
            letter.to_lowercase().to_string()
        }
    };
    bookname.to_string().chars().map(letter_tidier).collect()
}

mod parse {
    use super::tidy_note_line;
    use anyhow::Result;
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
        let re_title = Regex::new(r#"<h3.*>(.*)</h3>"#).unwrap();
        let title: String = re_title
            .captures_iter(&data)
            .take(1)
            .map(|x| x[1].to_string())
            .collect();
        let re_hi_or_note =
            Regex::new(r#"(?s)<span.*?id="(?:highlight|note)".*?>(.*?)</span>"#).unwrap();
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
}
