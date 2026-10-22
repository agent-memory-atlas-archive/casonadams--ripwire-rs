//! Source-line syntax for feature-flag harvesting: environment reads, preprocessor gates, CMake options.
#[derive(Clone, Copy)]
struct EnvLineSyntax {
    hash_comments: bool,
    char_literals: bool,
}

impl EnvLineSyntax {
    fn for_lang(lang: crate::domain::Lang) -> Self {
        Self {
            hash_comments: lang.semantics().uses_hash_comments(),
            char_literals: lang.semantics().has_lifetime_tick_syntax(),
        }
    }

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

fn env_name_at(rest: &str) -> Option<&str> {
    for call in ["getenv", "environ.get"] {
        if let Some(args) = rest.strip_prefix(call) {
            return quoted_env_name(args.trim_start().strip_prefix('(')?, b"\"");
        }
    }
    if let Some(sub) = rest.strip_prefix("environ[") {
        return quoted_env_name(sub, b"\"");
    }
    let tail = rest.strip_prefix("process.env")?;
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

/// Environment-variable reads on one source line, in order, deduplicated: `getenv("X")`,
/// `environ.get("X")`, `environ["X"]`, `process.env.X` and `process.env["X"]`, outside strings and comments.
pub fn extract_env_identifiers(line: &str, lang: crate::domain::Lang) -> Vec<String> {
    let mask = EnvLineSyntax::for_lang(lang).code_mask(line);
    let bytes = line.as_bytes();
    let mut names: Vec<String> = Vec::new();
    for (i, &b) in bytes.iter().enumerate() {
        let at_word_start =
            i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
        if !(mask[i] && at_word_start && matches!(b, b'g' | b'e' | b'p')) {
            continue;
        }
        if let Some(name) = env_name_at(&line[i..])
            && !names.iter().any(|n| n.as_str() == name)
        {
            names.push(name.to_string());
        }
    }
    names
}

/// A C-preprocessor settings gate: `#ifndef NAME` followed by `#define NAME VALUE` (an include guard has no value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsGate {
    pub name: String,
    pub default: String,
    pub dark: bool,
    pub define_idx: usize,
}

fn ifndef_name(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix("#ifndef ")?.trim_start();
    let end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    let name = &rest[..end];
    (!name.is_empty()).then_some(name)
}

#[must_use]
pub fn extract_settings_gate(lines: &[&str], idx: usize) -> Option<SettingsGate> {
    let name = ifndef_name(lines[idx])?;
    lines
        .iter()
        .enumerate()
        .skip(idx + 1)
        .take(3)
        .find_map(|(define_idx, line)| {
            let mut parts = line.trim().strip_prefix("#define ")?.split_whitespace();
            (parts.next()? == name).then_some(())?;
            let value = parts.next()?;
            Some(SettingsGate {
                name: name.to_string(),
                default: value.to_string(),
                dark: value == "0" || value.eq_ignore_ascii_case("false"),
                define_idx,
            })
        })
}

fn measure_preproc_region_loc(lines: &[&str], start_idx: usize) -> usize {
    let mut depth = 1usize;
    for (offset, &line) in lines.iter().skip(start_idx + 1).enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#if") {
            depth += 1;
        } else if trimmed.starts_with("#endif") {
            depth -= 1;
            if depth == 0 {
                return offset;
            }
        }
    }
    0
}

/// One identifier used in a `#if` / `#ifdef` / `#elif` condition, with the lines its region spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalUse {
    pub name: String,
    pub line_idx: usize,
    pub loc: usize,
}

#[must_use]
pub fn conditional_uses(lines: &[&str]) -> Vec<ConditionalUse> {
    let mut uses = Vec::new();
    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let Some(expr) = ["#if ", "#ifdef ", "#elif "]
            .iter()
            .find_map(|p| trimmed.strip_prefix(p))
        else {
            continue;
        };
        let loc = measure_preproc_region_loc(lines, line_idx);
        uses.extend(
            expr.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .filter(|t| {
                    t.starts_with(|c: char| c.is_alphabetic() || c == '_') && *t != "defined"
                })
                .map(|t| ConditionalUse {
                    name: t.to_string(),
                    line_idx,
                    loc,
                }),
        );
    }
    uses
}

pub fn extract_cmake_option(line: &str, path: &str) -> Option<(String, String, bool)> {
    if !path.ends_with(".cmake") && !path.ends_with("CMakeLists.txt") {
        return None;
    }
    let rest = line.trim().strip_prefix("option(")?;
    let inner = rest.trim_end_matches(')').trim();
    let name: String = inner
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        return None;
    }
    let is_on = inner
        .split_whitespace()
        .last()
        .is_some_and(|t| t.eq_ignore_ascii_case("ON"));
    let def_val = if is_on { "ON" } else { "OFF" };
    Some((name, def_val.to_string(), !is_on))
}
