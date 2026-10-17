use std::borrow::Cow;
use std::fmt::{self, Write as _};
use std::io::{self, Write};
/// Replace C0 control characters with space, preserving tabs, newlines, and carriage returns.
#[inline]
#[must_use]
pub fn xml_safe_byte(c: u8) -> u8 {
    if c < 0x20 && c != b'\t' && c != b'\n' && c != b'\r' {
        b' '
    } else {
        c
    }
}

/// Convert legal control characters to character entity references.
#[inline]
#[must_use]
pub fn xml_control_entity(c: char) -> Option<&'static str> {
    match c {
        '\t' => Some("&#9;"),
        '\n' => Some("&#10;"),
        '\r' => Some("&#13;"),
        _ => None,
    }
}

/// Convert a character requiring XML escaping into its replacement string.
#[inline]
#[must_use]
pub const fn xml_escaped_char(c: char) -> Option<&'static str> {
    match c {
        '&' => Some("&amp;"),
        '<' => Some("&lt;"),
        '>' => Some("&gt;"),
        '"' => Some("&quot;"),
        '\'' => Some("&apos;"),
        '\t' => Some("&#9;"),
        '\n' => Some("&#10;"),
        '\r' => Some("&#13;"),
        c if (c as u32) < 0x20 => Some(" "),
        _ => None,
    }
}

/// Check if a byte needs XML entity escaping or control character scrubbing.
#[inline]
#[must_use]
pub fn needs_escaping(c: u8) -> bool {
    matches!(c, b'&' | b'<' | b'>' | b'"' | b'\'' | 0..=0x1F)
}

/// Zero-allocation `Display` wrapper that streams XML-escaped characters to a formatter.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XmlEscaped<'a>(pub &'a str);

impl fmt::Display for XmlEscaped<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.0.bytes().any(needs_escaping) {
            return f.write_str(self.0);
        }
        for c in self.0.chars() {
            match xml_escaped_char(c) {
                Some(esc) => f.write_str(esc)?,
                None => f.write_char(c)?,
            }
        }
        Ok(())
    }
}

/// Extension trait for formatting string slices as XML-escaped values in `write!` / `format!`.
pub trait XmlEscapeExt {
    #[must_use]
    fn xml_escaped(&self) -> XmlEscaped<'_>;
}

impl<T: AsRef<str> + ?Sized> XmlEscapeExt for T {
    #[inline]
    fn xml_escaped(&self) -> XmlEscaped<'_> {
        XmlEscaped(self.as_ref())
    }
}

/// Escape XML entities (&, <, >, ", ') and control characters into a Cow string.
#[must_use]
pub fn escape_xml_str(s: &str) -> Cow<'_, str> {
    if !s.bytes().any(needs_escaping) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 16);
    let _ = write!(out, "{}", XmlEscaped(s));
    Cow::Owned(out)
}

/// Stream escaped XML directly to a writer without allocating a full string.
pub fn escape_xml_write<W: Write>(w: &mut W, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    if !bytes.iter().any(|&b| needs_escaping(b)) {
        return w.write_all(bytes);
    }

    for c in s.chars() {
        match c {
            '&' => w.write_all(b"&amp;")?,
            '<' => w.write_all(b"&lt;")?,
            '>' => w.write_all(b"&gt;")?,
            '"' => w.write_all(b"&quot;")?,
            '\'' => w.write_all(b"&apos;")?,
            '\t' => w.write_all(b"&#9;")?,
            '\n' => w.write_all(b"&#10;")?,
            '\r' => w.write_all(b"&#13;")?,
            c if (c as u32) < 0x20 => w.write_all(b" ")?,
            _ => {
                let mut buf = [0u8; 4];
                let encoded = c.encode_utf8(&mut buf);
                w.write_all(encoded.as_bytes())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_clean_string() {
        let clean = "clean_identifier_123";
        let res = escape_xml_str(clean);
        assert!(matches!(res, Cow::Borrowed(_)));
        assert_eq!(res, clean);

        let mut buf = Vec::new();
        escape_xml_write(&mut buf, clean).unwrap();
        assert_eq!(std::str::from_utf8(&buf).unwrap(), clean);
    }

    #[test]
    fn test_escape_special_entities() {
        let text = "<tag name=\"foo\" key='bar' flag=\"a & b\">";
        let escaped = escape_xml_str(text);
        assert!(matches!(escaped, Cow::Owned(_)));
        assert_eq!(
            escaped,
            "&lt;tag name=&quot;foo&quot; key=&apos;bar&apos; flag=&quot;a &amp; b&quot;&gt;"
        );

        let mut buf = Vec::new();
        escape_xml_write(&mut buf, text).unwrap();
        assert_eq!(std::str::from_utf8(&buf).unwrap(), escaped.as_ref());
    }

    #[test]
    fn test_escape_controls_and_c0() {
        let text = "line1\tline2\nline3\rline4\x01bell\x1f";
        let escaped = escape_xml_str(text);
        assert_eq!(escaped, "line1&#9;line2&#10;line3&#13;line4 bell ");
        assert_eq!(
            text.xml_escaped().to_string(),
            "line1&#9;line2&#10;line3&#13;line4 bell "
        );
    }
}
