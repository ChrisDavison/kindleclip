use std::collections::BTreeMap;
use std::path::PathBuf;

use regex::Regex;

type Result<T> = std::result::Result<T, Box<dyn ::std::error::Error>>;

fn parse_myclippings(filename: &str) -> Result<BTreeMap<String, Vec<String>>> {
    let mut output: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let boundary = "==========\r\n";
    let contents = std::fs::read_to_string(filename)?;
    let notes = contents.split(boundary);
    for note in notes {
        if let Some((title, tidied_note)) = parse_note(note) {
            if !output.contains_key(&title) {
                output.insert(title.clone(), Vec::new());
            }
            let entry = output
                .get_mut(&title)
                .expect("Should be impossible after insert above");
            entry.push(tidied_note);
        }
    }
    Ok(output)
}

fn parse_webexport(filename: &str) -> Result<BTreeMap<String, Vec<String>>> {
    let contents = std::fs::read_to_string(filename)?;
    let re_title = Regex::new(r#"<h3.*>(.*)</h3>"#).unwrap();
    let title: String = re_title
        .captures_iter(&contents)
        .take(1)
        .map(|x| x[1].to_string())
        .collect();
    let re_hi_or_note =
        Regex::new(r#"(?s)<span.*?id="(?:highlight|note)".*?>(.*?)</span>"#).unwrap();
    let mut output: BTreeMap<String, Vec<String>> = BTreeMap::new();
    output.insert(title.clone(), vec![]);
    for cap in re_hi_or_note.captures_iter(&contents) {
        let entry = output
            .get_mut(&title)
            .expect("Should be impossible after insert above");
        entry.push(cap[1].replace("\n", ""));
    }
    Ok(output)
}

fn tidy_note_line(line: impl ToString) -> String {
    let linestr = line.to_string();
    if linestr.starts_with("- Your Highlight") {
        "".to_string()
    } else if linestr.starts_with("- Your Note") {
        "NOTE FOR PREV: ".to_string()
    } else {
        linestr + "\n"
    }
}

fn parse_note(note: impl ToString) -> Option<(String, String)> {
    let lines: Vec<String> = note.to_string().lines().map(|x| x.to_string()).collect();
    let title: String = lines
        .iter()
        .take(1)
        .map(|x| x.trim().trim_start_matches("\u{feff}"))
        .collect();
    let tidied_note = lines.iter().skip(1).map(tidy_note_line).collect();
    if title.is_empty() {
        None
    } else {
        Some((title, tidied_note))
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

fn main() {
    let progname: String = std::env::args().take(1).collect();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        println!("usage: {} <clippings_fname> <output_dir>", progname);
        std::process::exit(1)
    }
    let clippings_fname = args[0].to_string();
    let outdir = PathBuf::from(&args[1]);
    if !outdir.is_dir() {
        if let Err(e) = std::fs::create_dir(&outdir) {
            eprintln!("Failed to create output dir {:?}: {}", outdir, e);
            std::process::exit(2);
        }
    }

    let ext = PathBuf::from(&clippings_fname)
        .extension()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let parser = match ext.as_ref() {
        "html" => parse_webexport,
        "txt" => parse_myclippings,
        _ => {
            println!("Unsupported file format.");
            println!("Want html saved from kindle library webpage,");
            println!("or 'My Clippings.txt' from kindle memory.");
            return;
        }
    };

    if let Ok(clippings) = parser(&clippings_fname) {
        for (book, notes) in clippings {
            let mut output_filename = outdir.clone();
            output_filename.push(bookname_to_filename(&book) + ".txt");
            if let Err(e) = std::fs::write(&output_filename, notes.join("\n")) {
                eprintln!("Failed to write file {:?}: {}", output_filename, e);
            }
        }
    }
}
