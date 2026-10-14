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

/// An XML writer that conditionally wraps an inner writer with `MinifyingWriter`
/// when `pretty` is false, or passes writes through directly when `pretty` is true.
pub enum XmlWriter<'a, W: Write> {
    Plain(&'a mut W),
    Minifying(MinifyingWriter<&'a mut W>),
}

impl<'a, W: Write> XmlWriter<'a, W> {
    pub fn new(writer: &'a mut W, pretty: bool) -> Self {
        if pretty {
            Self::Plain(writer)
        } else {
            Self::Minifying(MinifyingWriter::new(writer))
        }
    }
}

impl<'a, W: Write> Write for XmlWriter<'a, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(w) => w.write(buf),
            Self::Minifying(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(w) => w.flush(),
            Self::Minifying(w) => w.flush(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minifying_writer_strips_inter_tag_whitespace() {
        let input = "<!-- comment -->\n<root a=\"1\">\n  <child b=\"2\"/>\n  <child c=\"3\">hello</child>\n</root>\n";
        let mut out = Vec::new();
        {
            let mut writer = MinifyingWriter::new(&mut out);
            writer.write_all(input.as_bytes()).unwrap();
        }
        let res = String::from_utf8(out).unwrap();
        assert_eq!(
            res,
            "<!-- comment --><root a=\"1\"><child b=\"2\"/><child c=\"3\">hello</child></root>\n"
        );
    }
}
