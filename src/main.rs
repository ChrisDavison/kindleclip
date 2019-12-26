use std::collections::BTreeMap;
use std::path::PathBuf;

type Result<T> = std::result::Result<T, Box<dyn ::std::error::Error>>;

fn parse_clippings(filename: String) -> Result<BTreeMap<String, Vec<String>>> {
    let mut output: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let boundary = "==========\r\n";
    let contents = std::fs::read_to_string(filename)?;
    let notes = contents.split(boundary);
    for note in notes {
        let title: String = note.lines().take(1).collect();
        let title_tidy = title.trim().trim_start_matches("\u{feff}").to_string();
        let note: String = note
            .lines()
            .skip(2)
            .filter(|l| !l.contains("- Your"))
            .collect::<Vec<&str>>()
            .join("\n");
        if title_tidy.is_empty() {
            continue;
        }
        if !output.contains_key(&title_tidy) {
            output.insert(title_tidy.clone(), Vec::new());
        }
        let entry = output
            .get_mut(&title_tidy)
            .expect("Should be impossible after insert above");
        entry.push(note);
    }
    Ok(output)
}

fn bookname_to_filename(bookname: impl ToString) -> String {
    let bad_chars = vec!['(', ')', ',', ':'];
    bookname
        .to_string()
        .chars()
        .filter(|x| !bad_chars.contains(x))
        .map(|x| if x == ' ' { String::from("-") } else { x.to_lowercase().to_string() })
        .collect()
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
    if let Ok(clippings) = parse_clippings(clippings_fname) {
        for (book, notes) in clippings {
            let mut output_filename = outdir.clone();
            output_filename.push(bookname_to_filename(&book) + ".txt");
            if let Err(e) = std::fs::write(&output_filename, notes.join("\n")) {
                eprintln!("Failed to write file {:?}: {}", output_filename, e);
            }
        }
    }
}
