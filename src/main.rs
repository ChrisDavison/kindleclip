use anyhow::{anyhow, Context, Result};
use clap::Clap;
use std::path::PathBuf;

mod parse;
mod util;

/// Parse a kindle 'My Clippings.txt', or saved webpage.
#[derive(Clap, Debug)]
struct Opts {
    file: PathBuf,
    outdir: PathBuf,

    /// Prompt for which books' notes to export
    #[clap(short, long)]
    select: bool,
}

fn main() {
    if let Err(err) = try_main() {
        eprintln!("{}", err);
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let args = Opts::parse();

    if !args.outdir.is_dir() {
        std::fs::create_dir(&args.outdir)
            .with_context(|| anyhow!("Failed to create output dir {:?}", args.outdir))?;
    }
    dbg!(&args);

    let parser = match args.file.extension().and_then(|x| x.to_str()) {
        Some("html") => parse::webexport,
        Some("txt") => parse::myclippings,
        _ => {
            println!("Unsupported file format.");
            println!("Want html saved from kindle library webpage,");
            println!("or 'My Clippings.txt' from kindle storage.");
            return Err(anyhow!("Invalid file format"));
        }
    };

    let data =
        std::fs::read_to_string(&args.file).with_context(|| "Failed to read clippings file")?;

    if let Ok(clippings) = parser(&data) {
        let mut titles: Vec<String> = clippings.keys().map(|x| x.to_string()).collect();
        if args.select {
            titles = util::choose_from_list(&titles)?;
        }
        for title in titles {
            export_book_notes(&title, &clippings[&title], &args.outdir)?;
        }
    }
    Ok(())
}

fn export_book_notes(book: &str, notes: &[String], outdir: &PathBuf) -> Result<()> {
    let mut output_filename = outdir.clone();
    output_filename.push(title_as_filename(&book) + ".md");
    let header_and_notes = format!("# {}\n\n## Notes\n\n{}", book, notes.join("\n"));
    std::fs::write(&output_filename, header_and_notes)
        .with_context(|| format!("Failed to write file {:?}", output_filename))
}

fn title_as_filename(bookname: impl ToString) -> String {
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
