use crate::domain::Lang;

/// Two same-length views of a source text: `code` keeps only code bytes, `comments` keeps only
/// comment bytes. Every other byte becomes a space; newlines are kept so line numbers line up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskedSource {
    pub code: String,
    pub comments: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Region {
    Code,
    Comment,
    Literal,
}

struct Masker<'a> {
    src: &'a [u8],
    code: Vec<u8>,
    comments: Vec<u8>,
}

impl Masker<'_> {
    fn emit_until(&mut self, i: &mut usize, end: usize, region: Region) {
        while *i < end {
            let b = self.src[*i];
            let blank = if b == b'\n' { b'\n' } else { b' ' };
            self.code
                .push(if region == Region::Code { b } else { blank });
            self.comments
                .push(if region == Region::Comment { b } else { blank });
            *i += 1;
        }
    }

    fn line_end(&self, from: usize) -> usize {
        self.src[from..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(self.src.len(), |p| from + p)
    }

    fn block_end(&self, from: usize) -> usize {
        self.src[from + 2..]
            .windows(2)
            .position(|w| w == b"*/")
            .map_or(self.src.len(), |p| from + 2 + p + 2)
    }

    fn quoted_end(&self, from: usize) -> usize {
        let quote = self.src[from];
        let mut j = from + 1;
        while j < self.src.len() {
            match self.src[j] {
                b'\\' => j += 2,
                b if b == quote => return j + 1,
                _ => j += 1,
            }
        }
        self.src.len()
    }

    fn rust_char_end(&self, from: usize) -> Option<usize> {
        let rest = &self.src[from + 1..];
        if rest.first() == Some(&b'\\') {
            let p = rest.get(2..)?.iter().position(|&b| b == b'\'')?;
            return Some(from + 1 + 2 + p + 1);
        }
        let ch_len = match *rest.first()? {
            0x00..=0x7f => 1,
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            _ => 4,
        };
        (rest.get(ch_len) == Some(&b'\'')).then_some(from + 1 + ch_len + 1)
    }
}

fn opens_line_comment(src: &[u8], i: usize, hash_comments: bool) -> bool {
    src[i..].starts_with(b"//") || (hash_comments && src[i] == b'#')
}

/// Split `src` into code-only and comment-only views using `lang`'s comment and literal syntax.
#[must_use]
pub fn mask_source(src: &str, lang: Lang) -> MaskedSource {
    let bytes = src.as_bytes();
    let hash_comments = lang.semantics().uses_hash_comments();
    let rust = lang == Lang::Rust;
    let mut m = Masker {
        src: bytes,
        code: Vec::with_capacity(bytes.len()),
        comments: Vec::with_capacity(bytes.len()),
    };
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let (end, region) = if opens_line_comment(bytes, i, hash_comments) {
            (m.line_end(i), Region::Comment)
        } else if bytes[i..].starts_with(b"/*") {
            (m.block_end(i), Region::Comment)
        } else if b == b'"' || b == b'`' || (b == b'\'' && !rust) {
            (m.quoted_end(i), Region::Literal)
        } else if b == b'\'' {
            m.rust_char_end(i)
                .map_or((i + 1, Region::Code), |e| (e, Region::Literal))
        } else {
            (i + 1, Region::Code)
        };
        m.emit_until(&mut i, end, region);
    }
    MaskedSource {
        code: String::from_utf8_lossy(&m.code).into_owned(),
        comments: String::from_utf8_lossy(&m.comments).into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_separates_code_comments_and_literals_per_line() {
        let src = "let a = \"strcpy(\"; // TODO: fix\nstrcpy(x); /* catch {} */\n";
        let m = mask_source(src, Lang::C);
        assert_eq!(m.code.len(), src.len());
        let code: Vec<&str> = m.code.lines().collect();
        let comments: Vec<&str> = m.comments.lines().collect();
        assert_eq!(code.len(), 2);
        assert!(!code[0].contains("strcpy("));
        assert!(!code[0].contains("TODO"));
        assert!(comments[0].contains("TODO: fix"));
        assert!(code[1].contains("strcpy(x)"));
        assert!(!code[1].contains("catch"));
        assert!(comments[1].contains("catch {}"));
    }

    #[test]
    fn test_mask_rust_lifetimes_stay_code_and_chars_are_literals() {
        let src = "fn f<'a>(s: &'a str) -> char { 'x' } // todo!(\nlet q = '\\'';\n";
        let m = mask_source(src, Lang::Rust);
        assert!(m.code.contains("fn f<'a>(s: &'a str)"));
        assert!(!m.code.contains("'x'"));
        assert!(!m.code.contains("todo!("));
        assert!(m.comments.contains("todo!("));
        assert!(!m.code.lines().nth(1).unwrap().contains('\''));
    }

    #[test]
    fn test_mask_hash_comments_only_for_hash_languages() {
        let py = mask_source("x = 1  # FIXME: later\n", Lang::Python);
        assert!(py.comments.contains("FIXME: later"));
        let c = mask_source("#include <a.h>\n", Lang::C);
        assert!(c.code.contains("#include"));
    }
}
