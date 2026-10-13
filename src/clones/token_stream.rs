use crate::domain::Lang;

/// Check if word is a reserved control-flow, declaration, or structural keyword.
#[must_use]
pub fn is_keyword(w: &str) -> bool {
    matches!(
        w,
        "if" | "else"
            | "for"
            | "while"
            | "do"
            | "switch"
            | "case"
            | "default"
            | "break"
            | "continue"
            | "return"
            | "goto"
            | "struct"
            | "class"
            | "enum"
            | "union"
            | "namespace"
            | "template"
            | "typename"
            | "typedef"
            | "using"
            | "public"
            | "private"
            | "protected"
            | "virtual"
            | "override"
            | "final"
            | "const"
            | "constexpr"
            | "static"
            | "inline"
            | "extern"
            | "volatile"
            | "mut"
            | "fn"
            | "def"
            | "func"
            | "function"
            | "let"
            | "var"
            | "val"
            | "impl"
            | "trait"
            | "type"
            | "interface"
            | "package"
            | "import"
            | "export"
            | "from"
            | "as"
            | "match"
            | "loop"
            | "try"
            | "catch"
            | "throw"
            | "raise"
            | "except"
            | "finally"
            | "with"
            | "yield"
            | "async"
            | "await"
            | "true"
            | "false"
            | "null"
            | "nil"
            | "None"
            | "self"
            | "this"
            | "super"
    )
}

pub(crate) fn skip_whitespace(bytes: &[u8], i: &mut usize) {
    while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
        *i += 1;
    }
}

fn skip_slash_comment(bytes: &[u8], i: &mut usize) -> bool {
    if *i + 1 >= bytes.len() || bytes[*i] != b'/' {
        return false;
    }
    if bytes[*i + 1] == b'/' {
        *i += 2;
        while *i < bytes.len() && bytes[*i] != b'\n' {
            *i += 1;
        }
        true
    } else if bytes[*i + 1] == b'*' {
        *i += 2;
        while *i + 1 < bytes.len() && !(bytes[*i] == b'*' && bytes[*i + 1] == b'/') {
            *i += 1;
        }
        if *i + 1 < bytes.len() {
            *i += 2;
        }
        true
    } else {
        false
    }
}

fn skip_hash_comment(bytes: &[u8], i: &mut usize) -> bool {
    if *i < bytes.len() && bytes[*i] == b'#' {
        *i += 1;
        while *i < bytes.len() && bytes[*i] != b'\n' {
            *i += 1;
        }
        true
    } else {
        false
    }
}

fn skip_comment(bytes: &[u8], i: &mut usize, hash_comments: bool) -> bool {
    skip_slash_comment(bytes, i) || (hash_comments && skip_hash_comment(bytes, i))
}

pub(crate) fn scan_string_literal(bytes: &[u8], i: &mut usize) -> String {
    let quote = bytes[*i];
    let start = *i;
    *i += 1;
    while *i < bytes.len() {
        if bytes[*i] == b'\\' {
            *i += 2;
            continue;
        }
        if bytes[*i] == quote {
            *i += 1;
            break;
        }
        *i += 1;
    }
    String::from_utf8_lossy(&bytes[start..(*i).min(bytes.len())]).to_string()
}

pub(crate) fn scan_number_literal(bytes: &[u8], i: &mut usize) -> String {
    let start = *i;
    while *i < bytes.len() && (bytes[*i].is_ascii_alphanumeric() || bytes[*i] == b'.') {
        *i += 1;
    }
    String::from_utf8_lossy(&bytes[start..*i]).to_string()
}

pub(crate) fn scan_identifier_or_keyword(bytes: &[u8], i: &mut usize) -> String {
    let start = *i;
    while *i < bytes.len() && (bytes[*i].is_ascii_alphanumeric() || bytes[*i] == b'_') {
        *i += 1;
    }
    String::from_utf8_lossy(&bytes[start..*i]).to_string()
}

pub(crate) fn scan_operator(bytes: &[u8], i: &mut usize) -> String {
    let multi_ops: &[&[u8]] = &[
        b"-=", b"*=", b"/=", b"%=", b"&=", b"|=", b"^=", b"->", b"=>", b"::", b"..", b"++", b"--",
        b"??", b"?.",
    ];

    for op in multi_ops {
        if bytes[*i..].starts_with(op) {
            *i += op.len();
            return String::from_utf8_lossy(op).to_string();
        }
    }

    let start = *i;
    *i += 1;
    String::from_utf8_lossy(&bytes[start..*i]).to_string()
}

/// Tokenize symbol source body into raw and normalized token streams.
fn scan_next_token(
    bytes: &[u8],
    i: &mut usize,
    raw_tokens: &mut Vec<String>,
    norm_tokens: &mut Vec<String>,
) {
    let b = bytes[*i];
    if b == b'"' || b == b'\'' || b == b'`' {
        let s = scan_string_literal(bytes, i);
        raw_tokens.push(s);
        norm_tokens.push("$S".to_string());
    } else if b.is_ascii_digit() {
        let n = scan_number_literal(bytes, i);
        raw_tokens.push(n);
        norm_tokens.push("$N".to_string());
    } else if b.is_ascii_alphabetic() || b == b'_' {
        let word = scan_identifier_or_keyword(bytes, i);
        if is_keyword(&word) {
            raw_tokens.push(word.clone());
            norm_tokens.push(word);
        } else {
            raw_tokens.push(word);
            norm_tokens.push("$I".to_string());
        }
    } else {
        let op = scan_operator(bytes, i);
        raw_tokens.push(op.clone());
        norm_tokens.push(op);
    }
}

/// Tokenize symbol source body into raw and normalized token streams.
#[must_use]
pub fn tokenize_body(bytes: &[u8], lang: Lang) -> (Vec<String>, Vec<String>) {
    let hash_comments = matches!(
        lang,
        Lang::Python
            | Lang::Bash
            | Lang::Ruby
            | Lang::Toml
            | Lang::Yaml
            | Lang::Elixir
            | Lang::GDScript
    );

    let mut raw_tokens = Vec::new();
    let mut norm_tokens = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        skip_whitespace(bytes, &mut i);
        if skip_comment(bytes, &mut i, hash_comments) {
            continue;
        }
        if i >= bytes.len() {
            break;
        }
        scan_next_token(bytes, &mut i, &mut raw_tokens, &mut norm_tokens);
    }

    (raw_tokens, norm_tokens)
}

/// Compute 64-bit FNV-1a hash of a token sequence.
#[must_use]
pub fn hash_tokens(tokens: &[String]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for token in tokens {
        for byte in token.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0x20;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Compute sorted, unique k-gram fingerprint hashes for Type-3 detection.
#[must_use]
pub fn compute_kgrams(tokens: &[String], k: usize) -> Vec<u64> {
    if tokens.len() < k {
        return Vec::new();
    }

    let mut list: Vec<u64> = tokens.windows(k).map(hash_tokens).collect();
    list.sort_unstable();
    list.dedup();
    list
}

/// Compute 32-bit FNV-1a hash of a token byte slice.
#[must_use]
pub fn token_id(token: &[u8]) -> u32 {
    let mut hash = 0x811c9dc5u32;
    for &b in token {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

/// Compute 64-bit FNV-1a hash of a token id window.
#[must_use]
pub fn hash_token_window(window: &[u32]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &id in window {
        for b in id.to_le_bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0x20;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Compute sorted, unique k-gram fingerprint hashes from token ids.
#[must_use]
pub fn compute_kgrams_ids(ids: &[u32], k: usize) -> Vec<u64> {
    if ids.len() < k {
        return Vec::new();
    }
    let mut list: Vec<u64> = ids.windows(k).map(hash_token_window).collect();
    list.sort_unstable();
    list.dedup();
    list
}

/// Compute Jaccard similarity of two sorted, unique k-gram hash vectors.
#[must_use]
pub fn compute_jaccard(a: &[u64], b: &[u64]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let mut i = 0;
    let mut j = 0;
    let mut intersection = 0;

    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            intersection += 1;
            i += 1;
            j += 1;
        } else if a[i] < b[j] {
            i += 1;
        } else {
            j += 1;
        }
    }

    let union = a.len() + b.len() - intersection;
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Compute Longest Common Subsequence (LCS) length using 2-row dynamic programming.
#[must_use]
pub fn compute_lcs_len(a: &[u32], b: &[u32]) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }

    let mut prev = vec![0usize; b.len() + 1];
    let mut curr = vec![0usize; b.len() + 1];

    for &x in a {
        for (j, &y) in b.iter().enumerate() {
            curr[j + 1] = if x == y {
                prev[j] + 1
            } else {
                prev[j + 1].max(curr[j])
            };
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.fill(0);
    }

    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_normalization() {
        let code = b"fn add(x: i32, y: i32) -> i32 { return x + y + 10; }";
        let (raw, norm) = tokenize_body(code, Lang::Rust);
        assert!(norm.contains(&"$I".to_string()));
        assert!(norm.contains(&"$N".to_string()));
        assert!(norm.contains(&"return".to_string()));
        assert_eq!(raw[0], "fn");
    }

    #[test]
    fn test_skip_comments_and_strings() {
        let code =
            b"// line comment\n/* block comment */\n\"hello world\"\n'c'\n# hash comment\n123";
        let (raw, norm) = tokenize_body(code, Lang::Python);
        assert!(norm.contains(&"$S".to_string()));
        assert!(norm.contains(&"$N".to_string()));
        assert_eq!(raw[0], "\"hello world\"");
    }

    #[test]
    fn test_kgrams_and_jaccard() {
        let toks1: Vec<String> = vec!["fn", "$I", "(", "$I", ")", "{", "return", "$N", "}", ";"]
            .into_iter()
            .map(String::from)
            .collect();
        let toks2: Vec<String> = vec!["fn", "$I", "(", "$I", ")", "{", "return", "$N", "}", ";"]
            .into_iter()
            .map(String::from)
            .collect();

        let fp1 = compute_kgrams(&toks1, 5);
        let fp2 = compute_kgrams(&toks2, 5);
        let jaccard = compute_jaccard(&fp1, &fp2);
        assert!((jaccard - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_lcs_len() {
        let a = [1, 2, 3, 4, 5];
        let b = [1, 2, 4, 5];
        assert_eq!(compute_lcs_len(&a, &b), 4);
    }
}
