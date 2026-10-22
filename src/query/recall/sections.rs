/// One heading-delimited markdown section; lines are 1-based inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section {
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

impl Section {
    #[must_use]
    pub const fn bytes(&self) -> usize {
        self.end_byte - self.start_byte
    }
}
fn strip_indent(line: &str) -> Option<&str> {
    let t = line.trim_start_matches(' ');
    (line.len() - t.len() <= 3).then_some(t)
}

fn fence_marker(line: &str) -> Option<(u8, usize)> {
    let t = strip_indent(line)?;
    let bytes = t.as_bytes();
    let c = *bytes.first()?;
    if c != b'`' && c != b'~' {
        return None;
    }
    let n = bytes.iter().take_while(|&&x| x == c).count();
    (n >= 3).then_some((c, n))
}

#[must_use]
pub fn is_heading(line: &str) -> bool {
    let Some(t) = strip_indent(line) else {
        return false;
    };
    let rest = t.trim_start_matches('#');
    rest.len() < t.len() && (rest.is_empty() || rest.starts_with([' ', '\t', '\n', '\r']))
}

fn next_fence(open: Option<(u8, usize)>, line: &str) -> Option<(u8, usize)> {
    match (open, fence_marker(line)) {
        (None, marker) => marker,
        (Some((c, n)), Some((mc, mn))) if mc == c && mn >= n => None,
        (still_open, _) => still_open,
    }
}

/// Markdown sections from each `#` heading outside fenced code; text before the first heading is no section.
#[must_use]
pub fn split_sections(text: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut fence: Option<(u8, usize)> = None;
    let mut byte = 0usize;
    let mut line_no = 0usize;
    for line in text.split_inclusive('\n') {
        line_no += 1;
        let heading = fence.is_none() && is_heading(line);
        fence = next_fence(fence, line);
        if heading {
            if let Some(last) = sections.last_mut() {
                last.end_line = line_no - 1;
                last.end_byte = byte;
            }
            sections.push(Section {
                start_line: line_no,
                end_line: line_no,
                start_byte: byte,
                end_byte: byte,
            });
        }
        byte += line.len();
    }
    if let Some(last) = sections.last_mut() {
        last.end_line = line_no;
        last.end_byte = byte;
    }
    sections
}
