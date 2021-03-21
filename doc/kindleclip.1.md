% AN(1) Analyse Notes
%
% 2021-03-16


# NAME

an - analyse notes

# SYNOPSIS

    an complexity <files>...
    an headercount <files>...
    an size <files>...
    an structure <files>...
    an links <files>... [-l|--local]
    an untagged <files>...
    an tags <files>... [-t <tags>...] [-n <not-tags>...]

# DESCRIPTION

A bunch of features for analysing markdown notes that I find myself going back
to over and over again, wrapped up in a single tool.

*complexity* - heuristic on number of headers and lines of content within each
header.

*headercount* - count of all `#` headers

*size* - filesize in bytes. output is sorted.

*structure* - table of contents of each file

*links* - check all links (optionally only check local links)

*untagged* - show files without tags

*tags* - show tags for all files

# NOTES

A **tag** is something that matches the regexp **\@[a-zA-Z1-9]+**, 
i.e. **\@** followed by anything **alphanumeric**.

    @rust
    @linux
    @c99

# AUTHORS

Chris Davison <c.jr.davison@gmail.com>
