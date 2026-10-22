//! Feature-flag vocabulary shared by language plugins: what a plugin reports and how it declares env reads.

/// How a language reads an environment variable with a literal name.
#[derive(Debug, Clone, Copy)]
pub enum EnvRead {
    /// `name("X")`, e.g. `getenv("X")`.
    Call(&'static str),
    /// `name["X"]`, e.g. `environ["X"]`.
    Index(&'static str),
    /// `name.X` or `name["X"]`, e.g. `process.env.X`.
    Member(&'static str),
}

/// A feature-flag fact found in one file; `line` fields are 0-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagFact {
    Env {
        name: String,
        line: usize,
    },
    Option {
        kind: &'static str,
        name: String,
        default: String,
        dark: bool,
        line: usize,
    },
    Setting {
        name: String,
        default: String,
        dark: bool,
        line: usize,
        define_line: usize,
    },
    Conditional {
        name: String,
        line: usize,
        loc: usize,
    },
}

#[derive(Clone, Copy)]
struct EnvLineSyntax {
    hash_comments: bool,
    char_literals: bool,
}

impl EnvLineSyntax {
    fn starts_comment(self, bytes: &[u8], i: usize) -> bool {
        if self.hash_comments {
            bytes[i] == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace())
        } else {
            bytes[i] == b'/' && matches!(bytes.get(i + 1), Some(b'/' | b'*'))
        }
    }

    fn opens_quote(self, bytes: &[u8], i: usize) -> bool {
        match bytes[i] {
            b'"' | b'`' => true,
            b'\'' if self.char_literals => {
                bytes.get(i + 1) == Some(&b'\\') || bytes.get(i + 2) == Some(&b'\'')
            }
            b'\'' => i == 0 || !bytes[i - 1].is_ascii_alphanumeric(),
            _ => false,
        }
    }

    /// `true` at byte offsets that are code: outside string literals and before a line comment;
    /// a C-style block-comment continuation line (`* ...`) is all comment.
    fn code_mask(self, line: &str) -> Vec<bool> {
        let bytes = line.as_bytes();
        let mut mask = vec![false; bytes.len()];
        if !self.hash_comments && line.trim_start().starts_with('*') {
            return mask;
        }
        let mut quote: Option<u8> = None;
        let mut i = 0;
        while i < bytes.len() {
            match quote {
                Some(_) if bytes[i] == b'\\' => i += 1,
                Some(q) if bytes[i] == q => quote = None,
                Some(_) => {}
                None if self.starts_comment(bytes, i) => break,
                None if self.opens_quote(bytes, i) => quote = Some(bytes[i]),
                None => mask[i] = true,
            }
            i += 1;
        }
        mask
    }
}

fn is_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.len() <= 128
}

fn quoted_env_name<'a>(rest: &'a str, quotes: &[u8]) -> Option<&'a str> {
    let rest = rest.trim_start();
    let quote = *rest.as_bytes().first()?;
    if !quotes.contains(&quote) {
        return None;
    }
    let body = &rest[1..];
    let name = &body[..body.find(char::from(quote))?];
    is_env_name(name).then_some(name)
}

fn member_env_name(tail: &str) -> Option<&str> {
    if let Some(sub) = tail.strip_prefix('[') {
        return quoted_env_name(sub, b"\"'`");
    }
    let member = tail.strip_prefix('.')?;
    let end = member
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(member.len());
    let name = &member[..end];
    is_env_name(name).then_some(name)
}

fn env_name_at<'a>(rest: &'a str, reads: &[EnvRead]) -> Option<&'a str> {
    reads.iter().find_map(|read| match *read {
        EnvRead::Call(name) => quoted_env_name(
            rest.strip_prefix(name)?.trim_start().strip_prefix('(')?,
            b"\"",
        ),
        EnvRead::Index(name) => quoted_env_name(rest.strip_prefix(name)?.strip_prefix('[')?, b"\""),
        EnvRead::Member(name) => member_env_name(rest.strip_prefix(name)?),
    })
}

/// Environment-variable reads in `lines`, in order and deduplicated per line, outside strings and comments.
pub fn env_facts(
    lines: &[&str],
    hash_comments: bool,
    char_literals: bool,
    reads: &[EnvRead],
) -> Vec<FlagFact> {
    let syntax = EnvLineSyntax {
        hash_comments,
        char_literals,
    };
    let mut facts = Vec::new();
    if reads.is_empty() {
        return facts;
    }
    for (line_idx, line) in lines.iter().enumerate() {
        let mask = syntax.code_mask(line);
        let bytes = line.as_bytes();
        let mut seen: Vec<&str> = Vec::new();
        for i in (0..bytes.len()).filter(|&i| mask[i]) {
            let at_word_start =
                i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
            if let Some(name) = at_word_start
                .then(|| env_name_at(&line[i..], reads))
                .flatten()
                && !seen.contains(&name)
            {
                seen.push(name);
                facts.push(FlagFact::Env {
                    name: name.to_string(),
                    line: line_idx,
                });
            }
        }
    }
    facts
}
