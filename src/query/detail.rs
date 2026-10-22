use crate::domain::{IngestResult, Interner};
use crate::query::SourceReader;
use std::borrow::Cow;
use std::io::{self, Write};

pub struct BodyRequest<'a> {
    pub root: &'a str,
    pub symbols: &'a str,
    pub path_hint: Option<&'a str>,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub outline: bool,
    pub compress: bool,
}

pub fn skeletonize_body(body: &str) -> String {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut elided = false;
    for line in body.lines() {
        let opens = line.chars().filter(|&c| c == '{').count() as i32;
        let closes = line.chars().filter(|&c| c == '}').count() as i32;
        if depth <= 1 {
            out.push(line);
            elided = false;
        } else if !elided {
            out.push("    ...");
            elided = true;
        }
        depth += opens - closes;
        if depth <= 1 && closes > 0 && elided {
            out.push(line);
            elided = false;
        }
    }
    out.join("\n")
}

struct BodySelection<'a> {
    body: Cow<'a, str>,
    range: Option<(usize, usize, usize)>,
}

fn body_range<'a>(body: &'a str, request: &BodyRequest<'_>) -> io::Result<BodySelection<'a>> {
    if request.start_line.is_none() && request.end_line.is_none() {
        return Ok(BodySelection {
            body: Cow::Borrowed(body),
            range: None,
        });
    }
    let total = body.lines().count();
    let start = request.start_line.unwrap_or(1);
    let requested_end = request.end_line.unwrap_or(total);
    if start == 0 || requested_end < start || start > total {
        return Err(io::Error::other(format!(
            "invalid body line range {start}..{requested_end} for {total} lines"
        )));
    }
    let end = requested_end.min(total);
    let start_byte: usize = body
        .split_inclusive('\n')
        .take(start - 1)
        .map(str::len)
        .sum();
    let end_byte: usize = body.split_inclusive('\n').take(end).map(str::len).sum();
    let selected = &body[start_byte..end_byte];
    let selected = selected.strip_suffix('\n').unwrap_or(selected);
    let selected = selected.strip_suffix('\r').unwrap_or(selected);
    Ok(BodySelection {
        body: Cow::Borrowed(selected),
        range: Some((total, start, end)),
    })
}

pub fn execute_body_query<W: Write>(
    request: &BodyRequest<'_>,
    ingest: &IngestResult,
    interner: &Interner,
    sources: &dyn SourceReader,
    out: &mut W,
) -> io::Result<()> {
    let mut selected = Vec::new();
    for name in request
        .symbols
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        let ids = crate::navigate::find_symbols_by_name(name, ingest, interner);
        let previous = selected.len();
        selected.extend(ids.into_iter().filter(|&id| {
            request.path_hint.is_none_or(|hint| {
                ingest
                    .sym_file_path(&ingest.symbols[id as usize], interner)
                    .contains(hint)
            })
        }));
        if selected.len() == previous {
            return Err(io::Error::other(format!("symbol '{name}' not found")));
        }
    }
    if selected.is_empty() {
        return Err(io::Error::other("symbol name is required"));
    }
    let mut files = std::collections::HashMap::new();
    for &id in &selected {
        let sym = &ingest.symbols[id as usize];
        let path = ingest.sym_file_path(sym, interner);
        if let std::collections::hash_map::Entry::Vacant(entry) = files.entry(sym.file_id) {
            entry.insert(sources.read(path).map_err(|error| {
                io::Error::other(format!("cannot read file '{path}': {error}"))
            })?);
        }
    }
    let source_bodies: Vec<_> = selected
        .iter()
        .map(|&id| {
            let sym = &ingest.symbols[id as usize];
            sym.source_text(&files[&sym.file_id]).ok_or_else(|| {
                io::Error::other(format!(
                    "invalid source range for '{}' in '{}'",
                    interner.resolve(&sym.name),
                    ingest.sym_file_path(sym, interner)
                ))
            })
        })
        .collect::<io::Result<_>>()?;
    let mut bodies = Vec::with_capacity(selected.len());
    for (id, source) in selected.into_iter().zip(&source_bodies) {
        let sym = &ingest.symbols[id as usize];
        let path = ingest.sym_file_path(sym, interner);
        let BodySelection { body, range } = body_range(source, request)?;
        let body = if request.outline {
            Cow::Owned(skeletonize_body(&body))
        } else {
            body
        };
        let body = if request.compress {
            Cow::Owned(crate::clones::token_stream::compress_source_body(&body))
        } else {
            body
        };
        bodies.push((sym, interner.resolve(&sym.name), path, body, range));
    }
    let items: Vec<_> = bodies
        .iter()
        .map(
            |(symbol, name, path, body, range)| crate::serialize::workspace_xml::BodyItem {
                symbol,
                name,
                path,
                body,
                range: *range,
            },
        )
        .collect();
    crate::serialize::XmlReport::render_envelope(
        &crate::serialize::workspace_xml::BodyReport {
            root: request.root,
            items: &items,
        },
        out,
    )
}
