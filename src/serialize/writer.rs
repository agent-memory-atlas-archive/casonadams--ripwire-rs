use std::io::{self, Write};

#[derive(Copy, Clone, PartialEq, Eq)]
enum State {
    Normal,
    AfterTagClose,
    InWhitespace,
}

/// An XML streaming writer that strips inter-tag whitespace between element tags,
/// guaranteeing compact 1-line minified XML unless pretty formatting is enabled.
pub struct MinifyingWriter<W: Write> {
    inner: W,
    state: State,
    ws_buf: Vec<u8>,
}

impl<W: Write> MinifyingWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            state: State::Normal,
            ws_buf: Vec::with_capacity(64),
        }
    }
    fn handle_normal(&mut self, b: u8) -> io::Result<()> {
        self.inner.write_all(&[b])?;
        if b == b'>' {
            self.state = State::AfterTagClose;
        }
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
        match self.state {
            State::Normal => self.handle_normal(b),
            State::AfterTagClose => self.handle_after_tag(b),
            State::InWhitespace => self.handle_in_whitespace(b),
        }
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

fn unescape_xml_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn extract_next_attr(rest: &str, mut i: usize) -> Option<(String, serde_json::Value, usize)> {
    let bytes = rest.as_bytes();
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let key_start = i;
    while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let key = &rest[key_start..i];
    if i >= bytes.len() || bytes[i] != b'=' {
        return None;
    }
    i += 1;
    if i >= bytes.len() || bytes[i] != b'"' {
        return None;
    }
    i += 1;
    let val_start = i;
    while i < bytes.len() && bytes[i] != b'"' {
        i += 1;
    }
    let raw_val = unescape_xml_entities(&rest[val_start..i]);
    let json_val = if let Ok(n) = raw_val.parse::<i64>() {
        serde_json::Value::Number(n.into())
    } else {
        serde_json::Value::String(raw_val)
    };
    if i < bytes.len() {
        i += 1;
    }
    Some((key.to_string(), json_val, i))
}

fn parse_tag_attrs(tag_inner: &str) -> (&str, serde_json::Map<String, serde_json::Value>) {
    let trimmed = tag_inner.trim_end_matches('/').trim();
    let tag_name = trimmed.split_whitespace().next().unwrap_or("root");
    let rest = trimmed[tag_name.len()..].trim();
    let mut map = serde_json::Map::new();
    let mut i = 0;
    while let Some((k, v, next_i)) = extract_next_attr(rest, i) {
        map.insert(k, v);
        i = next_i;
    }
    (tag_name, map)
}

fn strip_xml_comments(mut s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    while let Some(start) = s.find("<!--") {
        out.push_str(&s[..start]);
        if let Some(end) = s[start..].find("-->") {
            s = &s[start + end + 3..];
        } else {
            break;
        }
    }
    out.push_str(s);
    out
}

/// Convert a Ripwire XML document string into a structured JSON object.
#[must_use]
pub fn xml_bytes_to_json_value(raw: &[u8]) -> serde_json::Value {
    let text = String::from_utf8_lossy(raw);
    let trimmed = text.trim();
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && let Ok(v) = serde_json::from_str(trimmed)
    {
        return v;
    }
    let clean_owned = strip_xml_comments(trimmed);
    let clean = clean_owned.trim();
    let Some(first_lt) = clean.find('<') else {
        return serde_json::json!({ "text": trimmed });
    };
    let Some(first_gt_rel) = clean[first_lt..].find('>') else {
        return serde_json::json!({ "text": trimmed });
    };
    let root_inner = &clean[first_lt + 1..first_lt + first_gt_rel];
    let (root_tag, mut root_map) = parse_tag_attrs(root_inner);
    root_map.insert(
        "tag".to_string(),
        serde_json::Value::String(root_tag.to_string()),
    );
    let mut items = Vec::new();
    let mut rest = &clean[first_lt + first_gt_rel + 1..];
    while let Some(lt) = rest.find('<') {
        let after = &rest[lt + 1..];
        let Some(gt) = after.find('>') else { break };
        if !after.starts_with('/') && !after.starts_with("![CDATA[") {
            let (child_tag, mut child_map) = parse_tag_attrs(&after[..gt]);
            if !child_map.is_empty() {
                child_map.insert(
                    "tag".to_string(),
                    serde_json::Value::String(child_tag.to_string()),
                );
                items.push(serde_json::Value::Object(child_map));
            }
        }
        rest = &after[gt + 1..];
    }
    root_map.insert("items".to_string(), serde_json::Value::Array(items));
    serde_json::Value::Object(root_map)
}

/// An XML writer that conditionally wraps an inner writer with `MinifyingWriter`
/// when `pretty` is false, passes writes through when `pretty` is true, or
/// converts emitted XML to JSON when `json` is true.
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
}

fn flush_no_comments<W: Write>(w: &mut W, acc: &mut Vec<u8>, pretty: bool) -> io::Result<()> {
    if acc.is_empty() {
        return w.flush();
    }
    let text = String::from_utf8_lossy(acc);
    let stripped = strip_xml_comments(&text);
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

fn flush_json<W: Write>(w: &mut W, acc: &mut Vec<u8>, pretty: bool) -> io::Result<()> {
    if !acc.is_empty() {
        let val = xml_bytes_to_json_value(acc);
        acc.clear();
        if pretty {
            serde_json::to_writer_pretty(&mut *w, &val)?;
        } else {
            serde_json::to_writer(&mut *w, &val)?;
        }
        writeln!(w)?;
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
            Self::NoComments(w, acc, pretty) => flush_no_comments(*w, acc, *pretty),
            Self::Json(w, acc, pretty) => flush_json(*w, acc, *pretty),
        }
    }
}
