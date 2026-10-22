use std::io::{self, Write};

#[derive(Copy, Clone, PartialEq, Eq)]
enum State {
    Normal,
    AfterTagClose,
    InWhitespace,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Markup {
    Text,
    Tag,
    Comment,
    Cdata,
    Processing,
}

/// A bounded lexical guard for streaming whitespace removal. XML validation
/// belongs to the buffered adapters; this only identifies protected regions.
struct LexicalGuard {
    markup: Markup,
    prefix: [u8; 9],
    prefix_len: usize,
    quote: u8,
    previous: [u8; 2],
}

impl LexicalGuard {
    fn new() -> Self {
        Self {
            markup: Markup::Text,
            prefix: [0; 9],
            prefix_len: 0,
            quote: 0,
            previous: [0; 2],
        }
    }

    fn advance(&mut self, byte: u8) -> bool {
        if self.markup == Markup::Text {
            if byte == b'<' {
                self.markup = Markup::Tag;
                self.prefix_len = 0;
                self.quote = 0;
            } else {
                return false;
            }
        }
        if self.prefix_len < self.prefix.len() {
            self.prefix[self.prefix_len] = byte;
            self.prefix_len += 1;
            self.markup = match &self.prefix[..self.prefix_len] {
                b"<!--" => Markup::Comment,
                b"<![CDATA[" => Markup::Cdata,
                b"<?" => Markup::Processing,
                _ => self.markup,
            };
        }
        let closes = match self.markup {
            Markup::Tag => self.closes_tag(byte),
            Markup::Comment => byte == b'>' && self.previous == *b"--",
            Markup::Cdata => byte == b'>' && self.previous == *b"]]",
            Markup::Processing => byte == b'>' && self.previous[1] == b'?',
            Markup::Text => false,
        };
        self.previous = [self.previous[1], byte];
        if closes {
            let tag_boundary = self.markup != Markup::Cdata;
            self.markup = Markup::Text;
            return tag_boundary;
        }
        false
    }

    fn closes_tag(&mut self, byte: u8) -> bool {
        if self.quote != 0 {
            if byte == self.quote {
                self.quote = 0;
            }
        } else if matches!(byte, b'\'' | b'"') {
            self.quote = byte;
        } else {
            return byte == b'>';
        }
        false
    }
}

/// An XML streaming writer that strips inter-tag whitespace while preserving
/// quoted attributes and source text, including multiline CDATA.
pub struct MinifyingWriter<W: Write> {
    inner: W,
    state: State,
    ws_buf: Vec<u8>,
    lexical: LexicalGuard,
}

impl<W: Write> MinifyingWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            state: State::Normal,
            ws_buf: Vec::with_capacity(64),
            lexical: LexicalGuard::new(),
        }
    }
    fn handle_normal(&mut self, b: u8) -> io::Result<()> {
        self.inner.write_all(&[b])?;
        Ok(())
    }

    fn handle_after_tag(&mut self, b: u8) -> io::Result<()> {
        if b.is_ascii_whitespace() {
            self.ws_buf.clear();
            self.ws_buf.push(b);
            self.state = State::InWhitespace;
        } else if b == b'<' {
            self.inner.write_all(b"<")?;
            self.state = State::Normal;
        } else {
            self.inner.write_all(&[b])?;
            self.state = State::Normal;
        }
        Ok(())
    }

    fn handle_in_whitespace(&mut self, b: u8) -> io::Result<()> {
        if b.is_ascii_whitespace() {
            self.ws_buf.push(b);
        } else if b == b'<' {
            self.ws_buf.clear();
            self.inner.write_all(b"<")?;
            self.state = State::Normal;
        } else {
            if !self.ws_buf.is_empty() {
                self.inner.write_all(&self.ws_buf)?;
                self.ws_buf.clear();
            }
            self.inner.write_all(&[b])?;
            self.state = State::Normal;
        }
        Ok(())
    }

    fn process_byte(&mut self, b: u8) -> io::Result<()> {
        if self.lexical.markup == Markup::Text {
            match self.state {
                State::Normal => self.handle_normal(b)?,
                State::AfterTagClose => self.handle_after_tag(b)?,
                State::InWhitespace => self.handle_in_whitespace(b)?,
            }
        } else {
            self.inner.write_all(&[b])?;
        }
        if self.lexical.advance(b) {
            self.state = State::AfterTagClose;
        } else if self.lexical.markup != Markup::Text {
            self.state = State::Normal;
        }
        Ok(())
    }
}

impl<W: Write> Write for MinifyingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for &b in buf {
            self.process_byte(b)?;
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if !self.ws_buf.is_empty() {
            self.inner.write_all(&self.ws_buf)?;
            self.ws_buf.clear();
        }
        self.state = State::Normal;
        self.inner.flush()
    }
}

impl<W: Write> Drop for MinifyingWriter<W> {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

fn invalid_xml(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

/// Remove actual XML comments without touching CDATA, entities, or source text.
/// Invalid XML is rejected rather than partially rewritten.
pub fn strip_xml_comments(s: &str) -> io::Result<String> {
    let document = roxmltree::Document::parse(s).map_err(invalid_xml)?;
    let mut out = String::with_capacity(s.len());
    let mut start = 0;
    for node in document.descendants().filter(|node| node.is_comment()) {
        let range = node.range();
        out.push_str(&s[start..range.start]);
        start = range.end;
    }
    out.push_str(&s[start..]);
    Ok(out)
}

fn expanded_name(namespace: Option<&str>, name: &str) -> String {
    match namespace {
        Some(namespace) => format!("{{{namespace}}}{name}"),
        None => name.to_owned(),
    }
}

fn attribute_value(value: &str) -> serde_json::Value {
    match value.parse::<i64>() {
        Ok(number) if number.to_string() == value => number.into(),
        _ => value.into(),
    }
}

fn element_value(node: roxmltree::Node<'_, '_>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let mut reserved = serde_json::Map::new();
    for attribute in node.attributes() {
        let key = expanded_name(attribute.namespace(), attribute.name());
        let value = attribute_value(attribute.value());
        if matches!(key.as_str(), "tag" | "items" | "text" | "attributes") {
            reserved.insert(key, value);
        } else {
            map.insert(key, value);
        }
    }
    if !reserved.is_empty() {
        map.insert("attributes".into(), reserved.into());
    }
    let name = node.tag_name();
    map.insert(
        "tag".into(),
        expanded_name(name.namespace(), name.name()).into(),
    );
    let has_elements = node.children().any(|child| child.is_element());
    let mut items = Vec::new();
    let mut text = String::new();
    for child in node.children() {
        if child.is_element() {
            items.push(element_value(child));
        } else if let Some(value) = child.text().filter(|_| child.is_text()) {
            if has_elements {
                items.push(value.into());
            } else {
                text.push_str(value);
            }
        }
    }
    if !text.is_empty() {
        map.insert("text".into(), text.into());
    }
    map.insert("items".into(), items.into());
    map.into()
}

/// Convert one complete XML report, pass through JSON, or wrap plain text.
///
/// Elements retain their attributes, `tag`, and nested `items`. Leaf content is
/// `text`; mixed content uses strings interleaved with elements in `items`, in
/// document order. Reserved-name attributes live only in `attributes`.
/// Namespaced names use `{namespace-uri}local-name`. Comments and processing
/// instructions are not report data. Non-XML text is returned as `{"text": ...}`
/// with surrounding whitespace trimmed. Invalid XML/UTF-8 returns `InvalidData`.
pub fn xml_bytes_to_json_value(raw: &[u8]) -> io::Result<serde_json::Value> {
    let text = std::str::from_utf8(raw).map_err(invalid_xml)?;
    if let Ok(value) = serde_json::from_str(text) {
        return Ok(value);
    }
    if !text.trim_start().starts_with('<') {
        return Ok(serde_json::json!({ "text": text.trim() }));
    }
    let document = roxmltree::Document::parse(text).map_err(invalid_xml)?;
    Ok(element_value(document.root_element()))
}

/// An XML writer that streams plain or minified XML, or buffers reports for
/// comment removal and JSON conversion.
///
/// Buffered reports are completed explicitly with [`Self::finish`]; `Write::flush`
/// only flushes the sink and is safe inside an unfinished XML wrapper. Comment
/// removal treats output beginning with `<` (after whitespace) as XML and validates
/// it; plain text and JSON are passed through byte-for-byte.
pub enum XmlWriter<'a, W: Write> {
    Plain(&'a mut W),
    Minifying(MinifyingWriter<&'a mut W>),
    NoComments(&'a mut W, Vec<u8>, bool),
    Json(&'a mut W, Vec<u8>, bool),
}

impl<'a, W: Write> XmlWriter<'a, W> {
    pub fn new(writer: &'a mut W, pretty: bool) -> Self {
        if pretty {
            Self::Plain(writer)
        } else {
            Self::Minifying(MinifyingWriter::new(writer))
        }
    }

    pub fn with_format(writer: &'a mut W, pretty: bool, json: bool) -> Self {
        Self::with_options(writer, pretty, json, false)
    }

    pub fn with_options(writer: &'a mut W, pretty: bool, json: bool, strip_comments: bool) -> Self {
        if json {
            Self::Json(writer, Vec::new(), pretty)
        } else if strip_comments {
            Self::NoComments(writer, Vec::new(), pretty)
        } else {
            Self::new(writer, pretty)
        }
    }

    /// Finalize a report; `Write::flush` only flushes the sink.
    /// Buffered output is validated and drained once; streaming output is flushed.
    pub fn finish(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(w) => w.flush(),
            Self::Minifying(w) => w.flush(),
            Self::NoComments(w, acc, pretty) => finish_no_comments(*w, acc, *pretty),
            Self::Json(w, acc, pretty) => finish_json(*w, acc, *pretty),
        }
    }
}

fn finish_no_comments<W: Write>(w: &mut W, acc: &mut Vec<u8>, pretty: bool) -> io::Result<()> {
    if acc.is_empty() {
        return w.flush();
    }
    let text = std::str::from_utf8(acc).map_err(invalid_xml)?;
    if !text.trim_start().starts_with('<') {
        w.write_all(acc)?;
        acc.clear();
        return w.flush();
    }
    let stripped = strip_xml_comments(text)?;
    let clean = stripped.trim_start();
    if pretty {
        w.write_all(clean.as_bytes())?;
    } else {
        let mut min = MinifyingWriter::new(&mut *w);
        min.write_all(clean.as_bytes())?;
        min.flush()?;
    }
    acc.clear();
    w.flush()
}

fn finish_json<W: Write>(w: &mut W, acc: &mut Vec<u8>, pretty: bool) -> io::Result<()> {
    if !acc.is_empty() {
        let val = xml_bytes_to_json_value(acc)?;
        if pretty {
            serde_json::to_writer_pretty(&mut *w, &val)?;
        } else {
            serde_json::to_writer(&mut *w, &val)?;
        }
        writeln!(w)?;
        acc.clear();
    }
    w.flush()
}

impl<'a, W: Write> Write for XmlWriter<'a, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(w) => w.write(buf),
            Self::Minifying(w) => w.write(buf),
            Self::NoComments(_, acc, _) | Self::Json(_, acc, _) => {
                acc.extend_from_slice(buf);
                Ok(buf.len())
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(w) => w.flush(),
            Self::Minifying(w) => w.flush(),
            Self::NoComments(w, _, _) | Self::Json(w, _, _) => w.flush(),
        }
    }
}
