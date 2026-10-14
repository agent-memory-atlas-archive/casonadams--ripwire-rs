use crate::domain::Lang;

pub const SKETCH_HASHES: usize = 128;
pub const SKETCH_MIN_MATCHES: u32 = 26;

const fn sketch_splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

const fn build_sketch_multipliers() -> [u64; SKETCH_HASHES] {
    let mut m = [0u64; SKETCH_HASHES];
    let mut j = 0;
    while j < SKETCH_HASHES {
        m[j] = sketch_splitmix64((j as u64) + 1) | 1;
        j += 1;
    }
    m
}

pub const SKETCH_MUL: [u64; SKETCH_HASHES] = build_sketch_multipliers();

/// Compute 128-byte deterministic MinHash sketch of a sorted k-gram fingerprint slice.
#[must_use]
pub fn compute_sketch(kgrams: &[u64]) -> [u8; SKETCH_HASHES] {
    let mut sig = [0xFFu8; SKETCH_HASHES];
    if kgrams.is_empty() {
        return sig;
    }
    for (j, &mul) in SKETCH_MUL.iter().enumerate() {
        let mut mn = u64::MAX;
        for &g in kgrams {
            let mut h = g.wrapping_mul(mul);
            h ^= h >> 32;
            if h < mn {
                mn = h;
            }
        }
        sig[j] = mn as u8;
    }
    sig
}

/// Count matching MinHash components between two 128-byte sketches.
#[must_use]
pub fn sketch_matches(a: &[u8; SKETCH_HASHES], b: &[u8; SKETCH_HASHES]) -> u32 {
    let mut matches = 0u32;
    for i in 0..SKETCH_HASHES {
        matches += u32::from(a[i] == b[i]);
    }
    matches
}

/// Check if word is a reserved control-flow, declaration, or structural keyword.
#[must_use]
pub fn is_keyword(w: &str) -> bool {
    is_keyword_bytes(w.as_bytes())
}

#[must_use]
pub fn is_keyword_bytes(w: &[u8]) -> bool {
    matches!(
        w,
        b"if"
            | b"else"
            | b"for"
            | b"while"
            | b"do"
            | b"switch"
            | b"case"
            | b"default"
            | b"break"
            | b"continue"
            | b"return"
            | b"goto"
            | b"struct"
            | b"class"
            | b"enum"
            | b"union"
            | b"namespace"
            | b"template"
            | b"typename"
            | b"typedef"
            | b"using"
            | b"public"
            | b"private"
            | b"protected"
            | b"virtual"
            | b"override"
            | b"final"
            | b"const"
            | b"constexpr"
            | b"static"
            | b"inline"
            | b"extern"
            | b"volatile"
            | b"mut"
            | b"fn"
            | b"def"
            | b"func"
            | b"function"
            | b"let"
            | b"var"
            | b"val"
            | b"impl"
            | b"trait"
            | b"type"
            | b"interface"
            | b"package"
            | b"import"
            | b"export"
            | b"from"
            | b"as"
            | b"match"
            | b"loop"
            | b"try"
            | b"catch"
            | b"throw"
            | b"raise"
            | b"except"
            | b"finally"
            | b"with"
            | b"yield"
            | b"async"
            | b"await"
            | b"true"
            | b"false"
            | b"null"
            | b"nil"
            | b"None"
            | b"self"
            | b"this"
            | b"super"
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

pub(crate) fn scan_string_slice<'a>(bytes: &'a [u8], i: &mut usize) -> &'a [u8] {
    let quote = bytes[*i];
    let start = *i;
    *i += 1;
    while *i < bytes.len() {
        if bytes[*i] == b'\\' {
            *i = (*i + 2).min(bytes.len());
            continue;
        }
        if bytes[*i] == quote {
            *i += 1;
            break;
        }
        *i += 1;
    }
    &bytes[start..(*i).min(bytes.len())]
}

pub(crate) fn scan_number_slice<'a>(bytes: &'a [u8], i: &mut usize) -> &'a [u8] {
    let start = *i;
    while *i < bytes.len() && (bytes[*i].is_ascii_alphanumeric() || bytes[*i] == b'.') {
        *i += 1;
    }
    &bytes[start..*i]
}

pub(crate) fn scan_ident_slice<'a>(bytes: &'a [u8], i: &mut usize) -> &'a [u8] {
    let start = *i;
    while *i < bytes.len() && (bytes[*i].is_ascii_alphanumeric() || bytes[*i] == b'_') {
        *i += 1;
    }
    &bytes[start..*i]
}

pub(crate) fn scan_operator_slice<'a>(bytes: &'a [u8], i: &mut usize) -> &'a [u8] {
    let multi_ops: &[&[u8]] = &[
        b"-=", b"*=", b"/=", b"%=", b"&=", b"|=", b"^=", b"->", b"=>", b"::", b"..", b"++", b"--",
        b"??", b"?.",
    ];
    for op in multi_ops {
        if bytes[*i..].starts_with(op) {
            *i += op.len();
            return op;
        }
    }
    let start = *i;
    *i += 1;
    &bytes[start..*i]
}

fn next_token_slices<'a>(bytes: &'a [u8], i: &mut usize) -> (&'a [u8], &'a [u8]) {
    let b = bytes[*i];
    if b == b'"' || b == b'\'' || b == b'`' {
        let raw = scan_string_slice(bytes, i);
        (raw, b"$S")
    } else if b.is_ascii_digit() {
        let raw = scan_number_slice(bytes, i);
        (raw, b"$N")
    } else if b.is_ascii_alphabetic() || b == b'_' {
        let raw = scan_ident_slice(bytes, i);
        let norm = if is_keyword_bytes(raw) { raw } else { b"$I" };
        (raw, norm)
    } else {
        let start = *i;
        *i += 1;
        let punct = &bytes[start..*i];
        (punct, punct)
    }
}

fn fnv1a_absorb_token(mut hash: u64, token: &[u8]) -> u64 {
    for &b in token {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash ^= 0x20;
    hash.wrapping_mul(0x1000_0000_01b3)
}

fn uses_hash_comments(lang: Lang) -> bool {
    matches!(
        lang,
        Lang::Python
            | Lang::Bash
            | Lang::Ruby
            | Lang::Toml
            | Lang::Yaml
            | Lang::Php
            | Lang::Elixir
            | Lang::GDScript
    )
}

/// Zero-allocation scanner returning `(raw_hash, norm_hash, norm_token_ids)`.
#[must_use]
pub fn scan_body_ids(bytes: &[u8], lang: Lang) -> (u64, u64, Vec<u32>) {
    let hash_comments = uses_hash_comments(lang);
    let mut raw_hash = 0xcbf2_9ce4_8422_2325u64;
    let mut norm_hash = 0xcbf2_9ce4_8422_2325u64;
    let mut norm_ids = Vec::with_capacity(bytes.len() / 6);
    let mut i = 0;

    while i < bytes.len() {
        skip_whitespace(bytes, &mut i);
        if i >= bytes.len() {
            break;
        }
        if skip_comment(bytes, &mut i, hash_comments) {
            continue;
        }
        let (raw_slice, norm_slice) = next_token_slices(bytes, &mut i);
        raw_hash = fnv1a_absorb_token(raw_hash, raw_slice);
        norm_hash = fnv1a_absorb_token(norm_hash, norm_slice);
        norm_ids.push(token_id(norm_slice));
    }

    (raw_hash, norm_hash, norm_ids)
}

/// Tokenize symbol source body into raw and normalized token streams.
#[must_use]
pub fn tokenize_body(bytes: &[u8], lang: Lang) -> (Vec<String>, Vec<String>) {
    let hash_comments = uses_hash_comments(lang);
    let mut raw_tokens = Vec::new();
    let mut norm_tokens = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        skip_whitespace(bytes, &mut i);
        if i >= bytes.len() {
            break;
        }
        if skip_comment(bytes, &mut i, hash_comments) {
            continue;
        }
        let (raw_slice, norm_slice) = next_token_slices(bytes, &mut i);
        raw_tokens.push(String::from_utf8_lossy(raw_slice).into_owned());
        norm_tokens.push(String::from_utf8_lossy(norm_slice).into_owned());
    }

    (raw_tokens, norm_tokens)
}

/// Compute 64-bit FNV-1a hash of a token sequence.
#[must_use]
pub fn hash_tokens(tokens: &[String]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for token in tokens {
        hash = fnv1a_absorb_token(hash, token.as_bytes());
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
    let mut hash = 0x811c_9dc5u32;
    for &b in token {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// Compute 64-bit FNV-1a hash of a token id window.
#[must_use]
pub fn hash_token_window(window: &[u32]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &id in window {
        for b in id.to_le_bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        hash ^= 0x20;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
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

fn trim_common_edges<'a>(a: &'a [u32], b: &'a [u32]) -> (&'a [u32], &'a [u32], usize) {
    let mut prefix = 0;
    let min_len = a.len().min(b.len());
    while prefix < min_len && a[prefix] == b[prefix] {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < (min_len - prefix) && a[a.len() - 1 - suffix] == b[b.len() - 1 - suffix] {
        suffix += 1;
    }
    (
        &a[prefix..a.len() - suffix],
        &b[prefix..b.len() - suffix],
        prefix + suffix,
    )
}

/// Compute Longest Common Subsequence (LCS) length using prefix/suffix trim and 1-row DP.
#[must_use]
pub fn compute_lcs_len(a: &[u32], b: &[u32]) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let (a_mid, b_mid, trimmed) = trim_common_edges(a, b);
    if a_mid.is_empty() || b_mid.is_empty() {
        return trimmed;
    }
    let (long, short) = if a_mid.len() >= b_mid.len() {
        (a_mid, b_mid)
    } else {
        (b_mid, a_mid)
    };
    let mut dp = vec![0u32; short.len() + 1];
    for &x in long {
        let mut prev_diag = 0u32;
        for (j, &y) in short.iter().enumerate() {
            let saved = dp[j + 1];
            dp[j + 1] = if x == y {
                prev_diag + 1
            } else {
                saved.max(dp[j])
            };
            prev_diag = saved;
        }
    }
    trimmed + (dp[short.len()] as usize)
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
        let (rh, nh, ids) = scan_body_ids(code, Lang::Rust);
        assert_eq!(rh, hash_tokens(&raw));
        assert_eq!(nh, hash_tokens(&norm));
        assert_eq!(ids.len(), norm.len());
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
    fn test_kgrams_jaccard_and_sketch() {
        let toks1: Vec<String> = vec!["fn", "$I", "(", "$I", ")", "{", "return", "$N", "}", ";"]
            .into_iter()
            .map(String::from)
            .collect();
        let toks2 = toks1.clone();
        let fp1 = compute_kgrams(&toks1, 5);
        let fp2 = compute_kgrams(&toks2, 5);
        let jaccard = compute_jaccard(&fp1, &fp2);
        assert!((jaccard - 1.0).abs() < 1e-6);
        let s1 = compute_sketch(&fp1);
        let s2 = compute_sketch(&fp2);
        assert_eq!(sketch_matches(&s1, &s2), SKETCH_HASHES as u32);
    }

    #[test]
    fn test_lcs_len() {
        let a = [1, 2, 3, 4, 5];
        let b = [1, 2, 4, 5];
        assert_eq!(compute_lcs_len(&a, &b), 4);
        assert_eq!(compute_lcs_len(&a, &a), 5);
    }
}
