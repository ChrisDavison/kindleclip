use anyhow::{anyhow, Context, Result};
use state::BookState;
use std::path::PathBuf;
use structopt::StructOpt;

mod export;
mod koreader_json;
mod koreader_lua;
mod model;
mod my_clippings;
mod server;
mod state;
mod util;
mod web_export;

/// Parse a kindle 'My Clippings.txt', a saved kindle library web page, or a
/// KOReader JSON highlight export or Lua metadata file.
#[derive(StructOpt, Debug)]
#[structopt(name = "kindleclip")]
pub struct Opts {
    /// Run the web server instead of exporting
    #[structopt(long)]
    pub serve: bool,

    /// (serve) Port to listen on
    #[structopt(long, default_value = "8080")]
    pub port: u16,

    /// (serve) Address to bind
    #[structopt(long, default_value = "127.0.0.1")]
    pub bind: String,

    /// (serve) State file location (defaults to beside the first source)
    #[structopt(long)]
    pub state: Option<PathBuf>,

    /// Prompt for which books' notes to export
    #[structopt(short, long)]
    select: bool,

    /// Filter for titles (implies select)
    #[structopt(long)]
    filter: Option<String>,

    /// Export as list, rather than paragraphs
    #[structopt(short, long)]
    list: bool,

    /// '<file> <outdir>' when exporting, one or more '<file>...' with --serve
    pub files: Vec<PathBuf>,
}

fn main() {
    if let Err(err) = try_main() {
        eprintln!("{}", err);
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let args = Opts::from_args();
    if args.serve {
        let runtime = tokio::runtime::Runtime::new()
            .with_context(|| "Failed to start tokio runtime")?;
        runtime.block_on(server::run(args))
    } else {
        export_mode(args)
    }
}

fn export_mode(args: Opts) -> Result<()> {
    if args.files.len() != 2 {
        return Err(anyhow!(
            "Expected exactly two arguments: <clipping_file> <output_dir>\n\
             Run with --serve to start the web server instead."
        ));
    }
    let file = args.files[0].clone();
    let outdir = args.files[1].clone();

    if !outdir.is_dir() {
        std::fs::create_dir(&outdir)
            .with_context(|| anyhow!("Failed to create output dir {:?}", outdir))?;
    }

    let data =
        std::fs::read_to_string(&file).with_context(|| "Failed to read clippings file")?;

    let mut books = model::parse_source(&file, &data)
        .with_context(|| "Failed to parse clippings.")?;

    // Books ordered by most-recent-entry, matching the previous behaviour.
    books.sort_by(|a, b| {
        (a.last_pos, a.title.as_str()).cmp(&(b.last_pos, b.title.as_str()))
    });
    let mut titles: Vec<String> = books.iter().map(|b| b.title.clone()).collect();
    if args.select || args.filter.is_some() {
        titles = util::choose_from_list(&titles, args.filter)?;
    }
    let chosen: std::collections::HashSet<String> = titles.into_iter().collect();
    let empty_state = BookState::default();
    let opts = export::ExportOpts {
        as_list: args.list,
        only_marked: false,
    };
    for book in books.iter().filter(|b| chosen.contains(&b.title)) {
        let body = export::render_book(book, &empty_state, &opts);
        let mut output_filename: PathBuf = outdir.clone();
        output_filename.push(format!("{}.md", book.filestem()));
        std::fs::write(&output_filename, body)
            .with_context(|| anyhow!("Failed to write file {:?}", output_filename))?;
    }
    Ok(())
}
