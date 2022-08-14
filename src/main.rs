use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

mod my_clippings;
mod note;
mod util;
mod web_export;

use note::Highlight;

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
        Some("html") | Some("htm") => web_export::parse,
        Some("txt") => my_clippings::parse,
        _ => {
            println!("Unsupported file format.");
            println!("Want html saved from kindle library webpage,");
            println!("or 'My Clippings.txt' from kindle storage.");
            return Err(anyhow!("Invalid file format"));
        }
    };

    let data =
        std::fs::read_to_string(&args.file).with_context(|| "Failed to read clippings file")?;

    let (clippings, mru_ordered_titles) =
        parser(&data).with_context(|| "Failed to parse clippings.")?;
    // let mut titles: Vec<String> = clippings.keys().map(|x| x.to_string()).collect();
    let mut titles = mru_ordered_titles.clone();
    if args.select || args.filter.is_some() {
        titles = util::choose_from_list(&mru_ordered_titles, args.filter)?;
    }
    for title in titles {
        export_book_notes(&title, &clippings[&title], &args.outdir, args.list)?;
    }
    Ok(())
}

fn export_book_notes(book: &str, notes: &[Highlight], outdir: &Path, as_list: bool) -> Result<()> {
    let (joiner, start) = if as_list { ("\n", "- ") } else { ("\n\n", "") };
    let filestem = notes[0].filestem();
    let notes = notes
        .iter()
        .map(|n| format!("{}{}", start, n))
        .collect::<Vec<String>>()
        .join(joiner);
    let mut output_filename: PathBuf = outdir.into();

    output_filename.push(filestem + ".md");
    let notes = format!("# {}\n\n## Notes\n\n{}", book, notes);
    std::fs::write(&output_filename, notes)
        .with_context(|| format!("Failed to write file {:?}", output_filename))
}
