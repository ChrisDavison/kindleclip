//! Read KOReader's serialized Lua tables as data, never as executable code.
use crate::model::*;
use anyhow::{bail, Context, Result};
use chrono::NaiveDateTime;
use serde_json::{Map, Value};

pub fn parse(data: &str) -> Result<Vec<Book>> {
    let mut reader = Reader {
        input: data,
        pos: 0,
    };
    reader.expect("return")?;
    let root = reader.value(0).context("Invalid KOReader Lua metadata")?;
    reader.skip()?;
    if reader.pos != data.len() {
        bail!("Unexpected trailing Lua code");
    }
    let title = string(&root["doc_props"], "title")
        .filter(|s| !s.trim().is_empty())
        .context("KOReader Lua metadata has no doc_props.title")?;
    let annotations = root
        .get("annotations")
        .and_then(Value::as_object)
        .context("KOReader Lua metadata has no annotations table")?;
    let key = merge_key(&title);
    let mut entries = annotations
        .iter()
        .map(|(k, v)| Ok((k.parse::<usize>().context("Invalid annotation index")?, v)))
        .collect::<Result<Vec<_>>>()?;
    entries.sort_by_key(|(i, _)| *i);
    let last_pos = entries.last().map_or(0, |(i, _)| *i);
    let mut clippings = Vec::new();
    for (_, entry) in entries {
        let text = string(entry, "text").unwrap_or_default();
        let note = string(entry, "note").filter(|s| !s.trim().is_empty());
        if text.trim().is_empty() && note.is_none() {
            continue;
        }
        let page = entry
            .get("pageno")
            .and_then(Value::as_i64)
            .or_else(|| entry.get("page").and_then(Value::as_i64));
        let added = string(entry, "datetime")
            .and_then(|s| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok())
            .map(|d| d.and_utc());
        clippings.push(Clipping {
            id: clipping_id(&key, &text),
            kind: if entry.get("pos0").is_some() {
                Kind::Highlight
            } else {
                Kind::Note
            },
            text,
            note,
            page,
            location: page.map(|p| format!("page {p}")),
            chapter: string(entry, "chapter"),
            added,
            source: Source::KoReader,
        });
    }
    Ok(vec![Book {
        id: fnv1a64(&key),
        title,
        author: string(&root["doc_props"], "authors"),
        clippings,
        last_pos,
    }])
}

fn string(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_owned)
}

struct Reader<'a> {
    input: &'a str,
    pos: usize,
}
impl Reader<'_> {
    fn rest(&self) -> &str {
        &self.input[self.pos..]
    }
    fn take(&mut self, s: &str) -> bool {
        if self.rest().starts_with(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }
    fn long_string(&mut self) -> Result<Option<String>> {
        let bytes = self.rest().as_bytes();
        if bytes.first() != Some(&b'[') {
            return Ok(None);
        }
        let mut n = 1;
        while bytes.get(n) == Some(&b'=') {
            n += 1;
        }
        if bytes.get(n) != Some(&b'[') {
            return Ok(None);
        }
        let end = format!("]{}]", "=".repeat(n - 1));
        self.pos += n + 1;
        let len = self
            .rest()
            .find(&end)
            .context("Unterminated Lua long string")?;
        let text = self.rest()[..len]
            .strip_prefix("\r\n")
            .or_else(|| self.rest()[..len].strip_prefix('\n'))
            .unwrap_or(&self.rest()[..len])
            .to_owned();
        self.pos += len + end.len();
        Ok(Some(text))
    }
    fn skip(&mut self) -> Result<()> {
        loop {
            while self.rest().starts_with(char::is_whitespace) {
                self.pos += self.rest().chars().next().unwrap().len_utf8();
            }
            if !self.take("--") {
                return Ok(());
            }
            if self.long_string()?.is_none() {
                self.pos += self.rest().find('\n').unwrap_or(self.rest().len());
            }
        }
    }
    fn expect(&mut self, s: &str) -> Result<()> {
        self.skip()?;
        if !self.take(s) {
            bail!("Expected {s:?} at byte {}", self.pos);
        }
        Ok(())
    }
    fn quoted(&mut self) -> Result<String> {
        let quote = self.rest().chars().next().context("Expected string")?;
        self.pos += 1;
        let mut out = Vec::new();
        loop {
            let c = self
                .rest()
                .chars()
                .next()
                .context("Unterminated Lua string")?;
            self.pos += c.len_utf8();
            if c == quote {
                return String::from_utf8(out).context("Invalid UTF-8 in Lua string");
            }
            if c != '\\' {
                let mut b = [0; 4];
                out.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
                continue;
            }
            let e = self.rest().chars().next().context("Unterminated escape")?;
            self.pos += e.len_utf8();
            match e {
                'a' => out.push(7),
                'b' => out.push(8),
                'f' => out.push(12),
                'n' | '\n' => out.push(b'\n'),
                'r' => out.push(b'\r'),
                't' => out.push(b'\t'),
                'v' => out.push(11),
                '\r' => {
                    self.take("\n");
                    out.push(b'\n');
                }
                '\\' | '"' | '\'' => out.push(e as u8),
                'z' => {
                    while self.rest().starts_with(char::is_whitespace) {
                        self.pos += self.rest().chars().next().unwrap().len_utf8();
                    }
                }
                '0'..='9' => {
                    let mut digits = e.to_string();
                    for _ in 0..2 {
                        if let Some(d @ '0'..='9') = self.rest().chars().next() {
                            digits.push(d);
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                    out.push(digits.parse::<u8>().context("Invalid decimal escape")?);
                }
                'x' => {
                    let hex = self.rest().get(..2).context("Invalid hex escape")?;
                    out.push(u8::from_str_radix(hex, 16).context("Invalid hex escape")?);
                    self.pos += 2;
                }
                _ => bail!("Unsupported Lua escape {e:?}"),
            }
        }
    }
    fn value(&mut self, depth: usize) -> Result<Value> {
        if depth > 128 {
            bail!("Lua tables nested too deeply");
        }
        self.skip()?;
        if let Some(s) = self.long_string()? {
            return Ok(Value::String(s));
        }
        if self.rest().starts_with(['"', '\'']) {
            return Ok(Value::String(self.quoted()?));
        }
        if self.take("{") {
            let mut table = Map::new();
            let mut index = 1;
            loop {
                self.skip()?;
                if self.take("}") {
                    return Ok(Value::Object(table));
                }
                let is_long = self
                    .rest()
                    .strip_prefix('[')
                    .is_some_and(|s| s.trim_start_matches('=').starts_with('['));
                let key = if !is_long && self.take("[") {
                    let k = self.value(depth + 1)?;
                    self.expect("]")?;
                    self.expect("=")?;
                    match k {
                        Value::String(s) => s,
                        Value::Number(n) => n.to_string(),
                        _ => bail!("Invalid table key"),
                    }
                } else {
                    let k = index.to_string();
                    index += 1;
                    k
                };
                table.insert(key, self.value(depth + 1)?);
                self.skip()?;
                if !self.take(",") && !self.take(";") {
                    self.expect("}")?;
                    return Ok(Value::Object(table));
                }
            }
        }
        let len = self
            .rest()
            .find(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '}' | ']'))
            .unwrap_or(self.rest().len());
        let token = &self.rest()[..len];
        let value = match token {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            "nil" => Value::Null,
            _ => Value::Number(
                token
                    .parse()
                    .with_context(|| format!("Unsupported Lua value {token:?}"))?,
            ),
        };
        self.pos += len;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_metadata_through_source_dispatch() {
        let data = r#"-- metadata.epub.lua
return {
 ["doc_props"] = {["title"] = "El padre", ["authors"] = "Díaz"},
 ["annotations"] = {
  [10] = {["text"] = "Bookmark", ["page"] = 30},
  [2] = {["text"] = "allana", ["note"] = "a note", ["pos0"] = "/body/text().53",
         ["page"] = "/body/text().53", ["pageno"] = 2, ["chapter"] = "Cubierta",
         ["datetime"] = "2026-07-07 00:01:39"},
  [3] = {["text"] = "", ["note"] = "  "},
 },
 ["unrelated"] = {true, false, -1.5, nil},
}"#;
        let books = parse_source(std::path::Path::new("metadata.epub.lua"), data).unwrap();
        let book = &books[0];
        assert_eq!(book.title, "El padre");
        assert_eq!(book.author.as_deref(), Some("Díaz"));
        assert_eq!(book.clippings.len(), 2);
        let c = &book.clippings[0];
        assert_eq!(c.kind, Kind::Highlight);
        assert_eq!(c.text, "allana");
        assert_eq!(c.note.as_deref(), Some("a note"));
        assert_eq!(c.page, Some(2));
        assert_eq!(c.chapter.as_deref(), Some("Cubierta"));
        assert_eq!(c.added.unwrap().to_rfc3339(), "2026-07-07T00:01:39+00:00");
        assert_eq!(c.id, clipping_id("El padre", "allana"));
        assert_eq!(book.clippings[1].kind, Kind::Note);
    }
    #[test]
    fn reads_lua_string_escapes_and_long_strings() {
        let mut r = Reader {
            input: r#"{"á\n\"\\\195\177", [=[
long text]=], 'line\
break'}"#,
            pos: 0,
        };
        let v = r.value(0).unwrap();
        assert_eq!(v["1"], "á\n\"\\ñ");
        assert_eq!(v["2"], "long text");
        assert_eq!(v["3"], "line\nbreak");
    }
    #[test]
    fn rejects_code_and_missing_metadata() {
        for s in [
            "return os.execute('touch bad')",
            "return {}",
            "return {",
            "return {} os.execute('bad')",
        ] {
            assert!(parse(s).is_err(), "{s}");
        }
    }
}
