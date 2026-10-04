# kindleclip 

[![Rust](https://github.com/ChrisDavison/kindleclip/actions/workflows/rust.yml/badge.svg)](https://github.com/ChrisDavison/kindleclip/actions/workflows/rust.yml)

Extract a file per book in your kindle `My Clippings.txt`.

Usage:

    kindleclip <CLIPPING_FILE> <OUTPUT_DIR>

This will go through all your kindle clippings and export notes to a file with
a somewhat-sanitised filename (lowercase, replace ' ' with '-', remove some
punctuation).

If the note was a *NOTE* (rather than a highlight), the text 'NOTE: ' is
exported above a note, so that it's easier to link highlight and note.

Flags:

- `--select` fuzzy-pick which books to export (uses skim)
- `--filter <query>` filter titles (implies select)
- `--list` export as a bulleted list rather than paragraphs

Sources can be a Kindle `My Clippings.txt` (`.txt`), a saved Kindle library
web page (`.html`), or a KOReader JSON highlight export (`.json`), or KOReader metadata (`.lua`).

## Web server

    kindleclip --serve <CLIPPING_FILE> [<MORE_FILES>...] [--port 8080] [--bind 127.0.0.1] [--state <path>]

Starts a local web UI listing every book found across the given sources.
Books with the same title are merged and identical clippings are
deduplicated, so a book highlighted on both native Kindle (My Clippings.txt)
and KOReader shows up once.

On the book page you can:

- mark clippings for export; "download marked" exports just those
- write an annotation per clipping, exported as a blockquote beneath it
- create named clusters of clippings, drag to reorder them, and export them
  as `##` sections of the book's markdown (leftover clippings land in an
  `## Unclustered` section)
- ignore clippings: they stay dimmed in the UI and are never exported

The book list can be sorted by recency (default) or alphabetically.

Downloads use the same markdown format as the CLI export. "download all"
zips one file per book.

### Where state lives

Annotations, marks, ignore flags and clusters persist in a JSON state file.
It defaults to sitting next to the first source file, e.g.
`My Clippings.kindleclip.json`, and `--state` moves it. Clippings are
identified by book title plus text, so re-running `--serve` with a newer
`My Clippings.txt` keeps everything you have written.

The `/import` page uploads additional source files without restarting; they
are stored in a `kindleclip-sources/` directory next to the state file and
reload on the next start.

### KOReader

On the device: Tools > Export highlights > Choose Formats, enable Json, then
"Export all notes in this book" or "Export all notes in your library" (the
library export contains every book in one file). Copy the file somewhere the
server can read it and pass it as a source, or upload it at `/import`.

You can also copy `metadata.epub.lua` (or another `metadata.*.lua` file)
from a book’s `.sdr` directory and import it directly. Highlights, attached
notes, pages, chapters and dates are preserved. Metadata must contain
`doc_props.title` and an `annotations` table. Lua is parsed as data and never
executed. Dates have no timezone in this format and are treated as UTC.

### Security

The server binds to 127.0.0.1 by default and has no authentication. It is a
local tool; do not expose it to untrusted networks.
