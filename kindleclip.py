#!/usr/bin/env python
import argparse
import collections
import pathlib
import enum
import re

from typing import List, Tuple


class HighlightType(enum.Enum):
    Highlight = enum.auto()
    Comment = enum.auto()

    def __str__(self) -> str:
        return self.__repr__()

    def __repr__(self) -> str:
        match self:
            case HighlightType.Comment:
                return "NOTE: "
            case _:
                return ""

    @staticmethod
    def from_str(s: str):
        if s.startswith("- Your Highlight"):
            return HighlightType.Highlight
        else:
            return HighlightType.Comment


class Note:
    title: str
    highlight_type: HighlightType
    pages: Tuple[int, int]
    date_added: str
    highlight: str

    def __init__(self, title, highlight_type, pages, date_added, highlight):
        self.title = title
        self.highlight_type = highlight_type
        self.pages = pages
        self.date_added = date_added
        self.highlight = highlight

    @staticmethod
    def from_str(s: str):
        if not s:
            return None
        lines = (line for line in s.splitlines())
        title = next(lines).replace("\ufeff", "").strip()

        re_metadata = re.compile(r".*(?!on page|at location) (\d+-*\d*).*Added.*, (.*)")
        metadata = next(lines)
        if "Your Bookmark" in metadata:
            return None
        m = re_metadata.search(metadata)
        if not m:
            print(s)
            return None
        if "-" in m.group(1):
            pstart, pend = [int(p) for p in m.group(1).split("-")]
        else:
            pstart, pend = int(m.group(1)), None
        date = m.group(2)
        htype = HighlightType.from_str(metadata)
        note = "".join([line.strip() for line in lines if line])
        return Note(title, htype, (pstart, pend), date, note)

    def __repr__(self) -> str:
        return f"{self.highlight_type}{self.highlight}"


def format_book(key: str, notes: List[Note], as_list: bool = False) -> str:
    if as_list:
        notestr = "\n".join(f"- {n}" for n in notes)
    else:
        notestr = "\n\n".join(f"{note}" for note in notes)

    for ch in "(),:":
        key = key.replace(ch, "")
    return f"# {key}\n\n## Notes\n\n{notestr}"


def book_filename(key: str) -> str:
    for ch in "(),:":
        key = key.replace(ch, "")
    return key.replace(" ", "-").lower() + ".md"


def parse_html_book_notes(filepath: pathlib.Path, outdir: pathlib.Path, as_list: bool, select: bool):
    existing_fn = pathlib.Path('~/.kindleclip.db').expanduser()
    existing = set(existing_fn.expanduser().read_text().splitlines()) if existing_fn.exists() else set()

    data = filepath.read_text()
    re_h3 = re.compile(r"<h3.*>(.*)</h3>")
    title = re_h3.search(data).group(1)
    if title in existing:
        print(f"Title `{title}` already in export list. Skipping.")
        return

    re_hi_or_note = re.compile("(?s)<span.*?id=\"(highlight|note)\".*?>(.*?)</span>")
    notes = []
    for match in re_hi_or_note.finditer(data):
        tidy = match.group(2).replace("\r", "").replace("\n", "")
        if tidy:
            highlight_type = HighlightType.Highlight if match.group(1) == "highlight" else HighlightType.Comment
            notes.append(Note(title, highlight_type, (0, 0), "", tidy))

    fn = book_filename(title)
    outpath = outdir / fn
    outpath.write_text(format_book(title, notes, as_list))
    existing.add(title)
    existing_fn.write_text('\n'.join(sorted(list(existing))))


def main(filepath: pathlib.Path, outdir: pathlib.Path, as_list: bool, select: bool):
    if not isinstance(filepath, pathlib.Path):
        filepath = pathlib.Path(filepath)
    if not isinstance(outdir, pathlib.Path):
        outdir = pathlib.Path(outdir)

    path = pathlib.Path(filepath)
    text = path.read_text()

    books = collections.defaultdict(list)
    if path.suffix in [".html", ".htm"]:
        parse_html_book_notes(filepath, outdir, as_list, select)
        return

    entries = text.replace("\r", "").split("==========\n")
    for e in entries:
        if note := Note.from_str(e):
            books[note.title].append(note)
    keys = list(books.keys())

    existing_fn = pathlib.Path('~/.kindleclip.db').expanduser()
    existing = set(existing_fn.expanduser().read_text().splitlines()) if existing_fn.exists() else set()
    to_export = [key for key in keys if key not in existing]
    if not to_export:
        print(f"Nothing new to export. db suggests {len(to_export)} books already exported.")
        return

    if select:
        from iterfzf import iterfzf
        to_export = iterfzf(to_export, multi=True, prompt="Books > ", cycle=True)

    if not to_export:
        print(f"Nothing selected to export.")
        return
    for key in to_export:
        fn = book_filename(key)
        outpath = outdir / fn
        outpath.write_text(format_book(key, books[key], as_list))
        existing.add(key)

    existing_fn.write_text('\n'.join(sorted(list(existing))))


def main_cli():
    parser = argparse.ArgumentParser()
    parser.add_argument("filepath", type=pathlib.Path)
    parser.add_argument(
        "outdir", nargs="?", type=pathlib.Path, default=pathlib.Path(".")
    )
    parser.add_argument('-l', '--list', action='store_true')
    parser.add_argument('-s', '--select', action='store_true')
    args = parser.parse_args()
    main(args.filepath, args.outdir, as_list=args.list, select=args.select)


if __name__ == "__main__":
    main_cli()
