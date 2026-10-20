use serde_json::Value;

const CSV_SAMPLE_ROWS: usize = 3;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocFormat {
    Markdown,
    Plain,
    Html,
    Csv,
    Notebook,
}
/// Document formats recall admits; anything else (`.txt`, `.json`, `.sql`, ...) is not a document.
#[must_use]
pub fn doc_format(path: &str) -> Option<DocFormat> {
    let ext = std::path::Path::new(path)
        .extension()?
        .to_str()?
        .to_ascii_lowercase();
    match ext.as_str() {
        "md" | "markdown" | "mdx" => Some(DocFormat::Markdown),
        "rst" | "adoc" | "org" => Some(DocFormat::Plain),
        "html" | "htm" => Some(DocFormat::Html),
        "csv" => Some(DocFormat::Csv),
        "ipynb" => Some(DocFormat::Notebook),
        _ => None,
    }
}

#[must_use]
pub fn render_doc(format: DocFormat, raw: &str) -> String {
    match format {
        DocFormat::Markdown | DocFormat::Plain => raw.to_string(),
        DocFormat::Html => html_to_text(raw),
        DocFormat::Csv => csv_to_text(raw),
        DocFormat::Notebook => notebook_to_text(raw),
    }
}

#[must_use]
pub fn notebook_to_text(raw: &str) -> String {
    let Ok(val) = serde_json::from_str::<Value>(raw) else {
        return raw.to_string();
    };
    let Some(cells) = val["cells"].as_array() else {
        return String::new();
    };
    let mut out = String::new();
    for cell in cells {
        if let Some(arr) = cell["source"].as_array() {
            for line in arr.iter().filter_map(Value::as_str) {
                out.push_str(line);
            }
            out.push('\n');
        } else if let Some(s) = cell["source"].as_str() {
            out.push_str(s);
            out.push('\n');
        }
    }
    out
}

/// Tags become word breaks, `script`/`style` bodies and comments vanish, entities decode.
#[must_use]
pub fn html_to_text(raw: &str) -> String {
    let mut text = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(lt) = rest.find('<') {
        text.push_str(&rest[..lt]);
        text.push(' ');
        rest = skip_tag(&rest[lt..]);
    }
    text.push_str(rest);
    collapse_whitespace(&decode_entities(&text))
}

fn skip_tag(s: &str) -> &str {
    if let Some(body) = s.strip_prefix("<!--") {
        return body.find("-->").map_or("", |i| &body[i + 3..]);
    }
    let Some(gt) = s.find('>') else {
        return "";
    };
    let after = &s[gt + 1..];
    let name: String = s[1..gt]
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    if name != "script" && name != "style" {
        return after;
    }
    let close = format!("</{name}");
    let Some(i) = after.to_ascii_lowercase().find(&close) else {
        return "";
    };
    let tail = &after[i..];
    tail.find('>').map_or("", |g| &tail[g + 1..])
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        let decoded = tail
            .find(';')
            .filter(|&i| i <= 10)
            .and_then(|i| decode_entity(&tail[1..i]).map(|c| (c, i)));
        if let Some((c, i)) = decoded {
            out.push(c);
            rest = &tail[i + 1..];
        } else {
            out.push('&');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

fn decode_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let num = name.strip_prefix('#')?;
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => num.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending: Option<char> = None;
    for c in s.chars() {
        if c.is_whitespace() {
            let newline = c == '\n' || pending == Some('\n');
            pending = Some(if newline { '\n' } else { ' ' });
            continue;
        }
        if let Some(p) = pending.take()
            && !out.is_empty()
        {
            out.push(p);
        }
        out.push(c);
    }
    out
}

/// Column header, row count, then the first few rows, fields space-joined.
#[must_use]
pub fn csv_to_text(raw: &str) -> String {
    let mut records = parse_csv(raw).into_iter();
    let header = records.next().unwrap_or_default();
    let rows: Vec<Vec<String>> = records.collect();
    let mut out = format!("CSV columns: {}\nrows: {}\n", header.join(" "), rows.len());
    for row in rows.iter().take(CSV_SAMPLE_ROWS) {
        out.push_str(&row.join(" "));
        out.push('\n');
    }
    out
}

fn parse_csv(raw: &str) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            (true, '"') => quoted = false,
            (false, '"') => quoted = true,
            (false, ',') => record.push(std::mem::take(&mut field)),
            (false, '\n') => {
                record.push(std::mem::take(&mut field));
                push_record(&mut records, std::mem::take(&mut record));
            }
            (false, '\r') => {}
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        push_record(&mut records, record);
    }
    records
}

fn push_record(records: &mut Vec<Vec<String>>, record: Vec<String>) {
    if record.iter().any(|f| !f.is_empty()) {
        records.push(record);
    }
}
