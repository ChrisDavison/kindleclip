# kindleclip 

Extract a file per book in your kindle `My Clippings.txt`.

Usage:

    kindleclip <CLIPPING_FILE> <OUTPUT_DIR>

This will go through all your kindle clippings and export notes to a file with
a somewhat-sanitised filename (lowercase, replace ' ' with '-', remove some
punctuation).

If the note was a *NOTE* (rather than a highlight), the text 'NOTE FOR PREV'
will be exported above a note, so that it's easier to link highlight and note.
