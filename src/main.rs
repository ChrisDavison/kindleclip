use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use structopt::StructOpt;

mod parse;
mod util;

/// Parse a kindle 'My Clippings.txt', or saved webpage.
#[derive(StructOpt, Debug)]
#[structopt(name = "kindleclip")]
struct Opts {
    file: PathBuf,
    outdir: PathBuf,

    /// Prompt for which books' notes to export
    #[structopt(short, long)]
    select: bool,

    /// Filter for titles (implies select)
    #[structopt(long)]
    filter: Option<String>,

    /// Export as list, rather than paragraphs
    #[structopt(short, long)]
    list: bool,

    /// markdown or org
    #[structopt(short, long)]
    format: Option<String>,
}

fn main() {
    if let Err(err) = try_main() {
        eprintln!("{}", err);
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let args = Opts::from_args();

    if !args.outdir.is_dir() {
        std::fs::create_dir(&args.outdir)
            .with_context(|| anyhow!("Failed to create output dir {:?}", args.outdir))?;
    }

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

    let clippings = parser(&data).with_context(|| "Failed to parse clippings.")?;
    let mut titles: Vec<String> = clippings.keys().map(|x| x.to_string()).collect();
    if args.select || args.filter.is_some() {
        titles = util::choose_from_list(&titles, args.filter)?;
    }
    let fmt = args.format;
    for title in titles {
        export_book_notes(
            &title,
            &clippings[&title],
            &args.outdir,
            fmt.clone(),
            args.list,
        )?;
    }
    Ok(())
}

fn export_book_notes(
    book: &str,
    notes: &[parse::Note],
    outdir: &PathBuf,
    format: Option<String>,
    as_list: bool,
) -> Result<()> {
    let (joiner, start, start_comment) = if as_list {
        ("\n", "- ", "    - NOTE: ")
    } else {
        ("\n\n", "", "NOTE: ")
    };
    let notes = notes
        .iter()
        .map(|n| match n {
            parse::Note::Highlight(h) => format!("{}{}", start, h),
            parse::Note::Comment(c) => format!("{}{}", start_comment, c),
        })
        .collect::<Vec<String>>()
        .join(joiner);
    let mut output_filename = outdir.clone();

    let header_and_notes = match format.as_deref() {
        Some("org") => {
            output_filename.push(title_as_filename(&book) + ".org");
            format!("#+TITLE: {}\n\n* Notes\n\n{}", book, notes)
        }
        Some("markdown" | "md") | _ => {
            output_filename.push(title_as_filename(&book) + ".md");
            format!("# {}\n\n## Notes\n\n{}", book, notes)
        }
    };
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
