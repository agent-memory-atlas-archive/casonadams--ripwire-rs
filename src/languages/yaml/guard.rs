/// Maximum safe YAML block nesting depth before parser scanner buffer risk.
pub const MAX_YAML_NEST_DEPTH: usize = 64;

fn scan_line_indent(bytes: &[u8], i: &mut usize) -> (u32, u32) {
    let mut col = 0u32;
    while *i < bytes.len() && (bytes[*i] == b' ' || bytes[*i] == b'\t') {
        col += 1;
        *i += 1;
    }
    let mut markers = 0u32;
    while *i + 1 < bytes.len()
        && (bytes[*i] == b'-' || bytes[*i] == b'?' || bytes[*i] == b':')
        && (bytes[*i + 1] == b' ' || bytes[*i + 1] == b'\t')
    {
        markers += 1;
        *i += 2;
        col += 2;
    }
    (col, markers)
}

fn is_blank_or_comment(bytes: &[u8], i: usize) -> bool {
    i >= bytes.len() || bytes[i] == b'\n' || bytes[i] == b'\r' || bytes[i] == b'#'
}

fn skip_to_next_line(bytes: &[u8], i: &mut usize) {
    while *i < bytes.len() && bytes[*i] != b'\n' {
        *i += 1;
    }
    if *i < bytes.len() {
        *i += 1;
    }
}

fn push_open_columns(
    open_cols: &mut [u32],
    open_count: &mut usize,
    base: u32,
    markers: u32,
) -> bool {
    while *open_count > 0 && open_cols[*open_count - 1] >= base {
        *open_count -= 1;
    }
    for m in 0..=markers {
        if *open_count >= open_cols.len() {
            return true;
        }
        open_cols[*open_count] = base + 2 * m;
        *open_count += 1;
    }
    2 * *open_count > MAX_YAML_NEST_DEPTH
}

/// Prescan YAML bytes to detect deeply nested block indentation that could overflow
/// tree-sitter-yaml's external scanner buffers.
#[must_use]
pub fn yaml_nests_too_deep(bytes: &[u8]) -> bool {
    let mut open_cols = [0u32; 128];
    let mut open_count = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        let (col, markers) = scan_line_indent(bytes, &mut i);
        let blank = is_blank_or_comment(bytes, i);
        if (!blank || markers > 0)
            && push_open_columns(
                &mut open_cols,
                &mut open_count,
                col.saturating_sub(2 * markers),
                markers,
            )
        {
            return true;
        }
        skip_to_next_line(bytes, &mut i);
    }
    false
}
