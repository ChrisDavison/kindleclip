use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use structopt::StructOpt;

mod my_clippings;
mod note;
mod util;
mod web_export;

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

    let clippings = parser(&data).with_context(|| "Failed to parse clippings.")?;
    let mut title_and_mru: Vec<_> = clippings
        .iter()
        .map(|(title, notes)| (notes.mru_indice, title))
        .collect();
    title_and_mru.sort();
    let mut titles: Vec<String> = title_and_mru
        .iter()
        .map(|(_, title)| title.to_string())
        .collect();
    if args.select || args.filter.is_some() {
        titles = util::choose_from_list(&titles, args.filter)?;
    }
    for title in titles {
        clippings[&title].export(&args.outdir, args.list)?;
    }
    Ok(())
}
