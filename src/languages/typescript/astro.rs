/// Result of extracting Astro frontmatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AstroFrontmatter {
    /// Frontmatter extracted: (content_start_byte, content_end_byte, content_start_row).
    Ok {
        start_byte: usize,
        end_byte: usize,
        start_row: u32,
    },
    /// Template-only Astro file without frontmatter.
    None,
    /// Opening fence present without a closing fence.
    Unterminated,
}

fn is_fence_tail_blank(src: &[u8], from: usize, to: usize) -> bool {
    src[from..to]
        .iter()
        .all(|&b| b == b' ' || b == b'\t' || b == b'\r')
}

fn is_fence_line(src: &[u8], line_start: usize, line_end: usize) -> bool {
    let len = line_end.saturating_sub(line_start);
    len >= 3
        && &src[line_start..line_start + 3] == b"---"
        && is_fence_tail_blank(src, line_start + 3, line_end)
}

fn skip_blank_lines(src: &[u8], pos: &mut usize) -> u32 {
    let mut rows = 0;
    while *pos < src.len() {
        let nl = src[*pos..]
            .iter()
            .position(|&b| b == b'\n')
            .map(|offset| *pos + offset)
            .unwrap_or(src.len());
        if is_fence_tail_blank(src, *pos, nl) {
            *pos = if nl < src.len() { nl + 1 } else { nl };
            rows += 1;
        } else {
            break;
        }
    }
    rows
}

/// Find frontmatter range in `.astro` file source bytes.
#[must_use]
pub fn extract_astro_frontmatter(src: &[u8]) -> AstroFrontmatter {
    let mut pos = 0;
    if src.starts_with(b"\xEF\xBB\xBF") {
        pos = 3;
    }
    let fence_row = skip_blank_lines(src, &mut pos);
    if pos >= src.len() {
        return AstroFrontmatter::None;
    }
    let open_end = match src[pos..].iter().position(|&b| b == b'\n') {
        Some(offset) => pos + offset,
        None => return AstroFrontmatter::None,
    };
    if !is_fence_line(src, pos, open_end) {
        return AstroFrontmatter::None;
    }

    let content_start = open_end + 1;
    let content_row = fence_row + 1;
    let mut line_start = content_start;

    while line_start <= src.len() {
        let nl = src[line_start..]
            .iter()
            .position(|&b| b == b'\n')
            .map(|offset| line_start + offset);
        let line_end = nl.unwrap_or(src.len());
        if is_fence_line(src, line_start, line_end) {
            if line_start <= content_start {
                return AstroFrontmatter::None;
            }
            return AstroFrontmatter::Ok {
                start_byte: content_start,
                end_byte: line_start,
                start_row: content_row,
            };
        }
        if nl.is_none() {
            break;
        }
        line_start = line_end + 1;
    }
    AstroFrontmatter::Unterminated
}
