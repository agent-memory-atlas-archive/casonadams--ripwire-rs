fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

pub fn matching_close(code: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, b) in code.bytes().enumerate().skip(open) {
        if b == b'(' {
            depth += 1;
        }
        if b == b')' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

pub fn call_positions<'a>(code: &'a str, name: &'a str) -> impl Iterator<Item = usize> + 'a {
    let bytes = code.as_bytes();
    code.match_indices(name).map(|(i, _)| i).filter(move |&i| {
        let end = i + name.len();
        let boundary = i == 0 || !is_word_byte(bytes[i - 1]);
        let qualified = i > 0 && matches!(bytes[i - 1], b'.' | b':' | b'>');
        boundary && !qualified && code[end..].trim_start().starts_with('(')
    })
}

pub fn call_end(code: &str, start: usize, name: &str) -> usize {
    let after = start + name.len();
    let open = after + code[after..].find('(').unwrap_or(0);
    matching_close(code, open).map_or(after, |c| c + 1)
}

pub fn decimal_value(token: &str) -> Option<f64> {
    let lower = token.to_ascii_lowercase();
    if lower.starts_with("0x") || lower.starts_with("0b") {
        return None;
    }
    let body = lower.trim_end_matches(['u', 'l', 'f', 'z']);
    body.replace('_', "").parse::<f64>().ok()
}
