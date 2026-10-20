mod format;
mod score;
mod sections;

pub use format::{DocFormat, csv_to_text, doc_format, html_to_text, notebook_to_text, render_doc};
pub use score::{is_generated, query_terms};
pub use sections::{Section, is_heading, split_sections};

use score::{TermStats, average_len, bm25, idf_weights, unit_stats};
use serde_json::{Value, json};
use std::cmp::Ordering;

pub const DEFAULT_TOP_K: usize = 8;
pub const DEFAULT_MAX_TOKENS: usize = 8000;
/// Upstream's body ceiling: 2.5 bytes/token less a 15.1% reserve for headers and notes.
const BODY_BYTES_PER_TOKEN: f64 = 2.1225;
/// Upstream displays relevance as 0.3 x BM25.
const RELEVANCE_SCALE: f64 = 0.3;
const MIN_SLICE_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallInput {
    pub path: String,
    pub format: DocFormat,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecallOptions {
    pub top_k: usize,
    pub max_tokens: usize,
}

impl Default for RecallOptions {
    fn default() -> Self {
        Self {
            top_k: DEFAULT_TOP_K,
            max_tokens: DEFAULT_MAX_TOKENS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionsNote {
    pub selected: usize,
    pub matched: usize,
    pub total: usize,
    pub spans: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServedDoc {
    pub path: String,
    pub relevance: f64,
    pub generated: bool,
    pub whole_bytes: usize,
    pub body: String,
    pub sections: Option<SectionsNote>,
    pub truncated: Option<(usize, usize)>,
}

impl ServedDoc {
    #[must_use]
    pub fn is_cut(&self) -> bool {
        self.truncated.is_some()
            || self
                .sections
                .as_ref()
                .is_some_and(|s| s.selected < s.matched)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecallOutcome {
    pub documents: usize,
    pub relevant: usize,
    pub generated_demoted: usize,
    pub omitted_top_k: usize,
    pub omitted_budget: usize,
    pub top_k: usize,
    pub max_tokens: usize,
    pub budget_bytes: usize,
    pub share_bytes: Option<usize>,
    pub docs: Vec<ServedDoc>,
}

impl RecallOutcome {
    #[must_use]
    pub fn truncated(&self) -> usize {
        self.docs.iter().filter(|d| d.is_cut()).count()
    }

    #[must_use]
    pub fn to_json(&self, task: &str) -> Value {
        let docs: Vec<Value> = self.docs.iter().map(served_doc_json).collect();
        let mut val = json!({
            "task": task,
            "documents": self.documents,
            "relevant": self.relevant,
            "shown": self.docs.len(),
            "top_k": self.top_k,
            "default_top_k": DEFAULT_TOP_K,
            "max_tokens": self.max_tokens,
            "budget_bytes": self.budget_bytes,
            "generated_demoted": self.generated_demoted,
            "truncated": self.truncated(),
            "omitted_top_k": self.omitted_top_k,
            "omitted_budget": self.omitted_budget,
            "capped": self.omitted_top_k + self.omitted_budget > 0 || self.truncated() > 0,
            "count": docs.len(),
            "docs": docs,
        });
        if let Some(share) = self.share_bytes {
            val["share_bytes"] = json!(share);
        }
        val
    }
}

fn served_doc_json(d: &ServedDoc) -> Value {
    let mut val = json!({
        "path": d.path,
        "score": (d.relevance * 1000.0).round() / 1000.0,
        "generated": d.generated,
        "whole_bytes": d.whole_bytes,
        "content": d.body,
    });
    if let Some(s) = &d.sections {
        val["sections"] = json!({
            "selected": s.selected,
            "matched": s.matched,
            "total": s.total,
            "lines": format_spans(&s.spans),
            "dropped_by_budget": s.matched - s.selected,
        });
    }
    if let Some((shown, of)) = d.truncated {
        val["truncated"] = json!({ "shown_bytes": shown, "of_bytes": of });
    }
    val
}

#[must_use]
pub fn format_spans(spans: &[(usize, usize)]) -> String {
    spans
        .iter()
        .map(|(a, b)| format!("{a}-{b}"))
        .collect::<Vec<_>>()
        .join(",")
}

struct Candidate<'a> {
    input: &'a RecallInput,
    score: f64,
    generated: bool,
    sections: Vec<Section>,
    hits: Vec<(usize, f64)>,
}

impl Candidate<'_> {
    fn payload(&self) -> usize {
        if self.hits.is_empty() {
            self.input.text.len()
        } else {
            self.hits
                .iter()
                .map(|&(i, _)| self.sections[i].bytes())
                .sum()
        }
    }
}

fn rank_order(a: &Candidate<'_>, b: &Candidate<'_>) -> Ordering {
    a.generated
        .cmp(&b.generated)
        .then_with(|| b.score.total_cmp(&a.score))
        .then_with(|| a.input.path.cmp(&b.input.path))
}

/// Scoring units of one doc: the lead text (whole doc when headless) as `None`, then each section.
struct DocUnits {
    sections: Vec<Section>,
    units: Vec<(Option<usize>, TermStats)>,
}

fn doc_units(input: &RecallInput, terms: &[String]) -> DocUnits {
    let text = &input.text;
    let sections = if input.format == DocFormat::Markdown {
        split_sections(text)
    } else {
        Vec::new()
    };
    let lead = &text[..sections.first().map_or(text.len(), |s| s.start_byte)];
    let mut units = Vec::with_capacity(sections.len() + 1);
    if sections.is_empty() || !lead.trim().is_empty() {
        units.push((None, unit_stats("", lead, &input.path, terms)));
    }
    for (i, s) in sections.iter().enumerate() {
        let chunk = &text[s.start_byte..s.end_byte];
        let (heading, body) = chunk.split_once('\n').unwrap_or((chunk, ""));
        let mut st = unit_stats(heading, body, &input.path, terms);
        let trim = heading.trim_start_matches(' ');
        st.title_preamble =
            i == 0 && sections.len() > 1 && trim.starts_with('#') && !trim.starts_with("##");
        units.push((Some(i), st));
    }
    DocUnits { sections, units }
}

fn score_doc<'a>(
    input: &'a RecallInput,
    doc: DocUnits,
    idf: &[f64],
    avg_len: f64,
) -> Option<Candidate<'a>> {
    let scored: Vec<(Option<usize>, f64)> = doc
        .units
        .iter()
        .filter(|(_, st)| st.matches())
        .map(|(sec, st)| (*sec, bm25(st, idf, avg_len)))
        .collect();
    let best = scored.iter().map(|&(_, s)| s).max_by(f64::total_cmp)?;
    Some(Candidate {
        input,
        score: RELEVANCE_SCALE * best,
        generated: is_generated(&input.text),
        hits: scored
            .iter()
            .filter_map(|&(sec, s)| sec.map(|i| (i, s)))
            .collect(),
        sections: doc.sections,
    })
}

fn ranked_candidates<'a>(inputs: &'a [RecallInput], terms: &[String]) -> Vec<Candidate<'a>> {
    let docs: Vec<DocUnits> = inputs.iter().map(|d| doc_units(d, terms)).collect();
    let all: Vec<&TermStats> = docs
        .iter()
        .flat_map(|d| d.units.iter().map(|(_, st)| st))
        .collect();
    let idf = idf_weights(&all, terms.len());
    let avg_len = average_len(&all);
    drop(all);
    let mut out: Vec<Candidate<'a>> = inputs
        .iter()
        .zip(docs)
        .filter_map(|(input, doc)| score_doc(input, doc, &idf, avg_len))
        .collect();
    out.sort_by(rank_order);
    out
}

/// The largest per-doc allowance such that every doc gets `min(payload, level)` within `budget`.
fn water_level(payloads: &[usize], budget: usize) -> usize {
    let mut sorted = payloads.to_vec();
    sorted.sort_unstable();
    let mut remaining = budget;
    let mut left = sorted.len();
    for p in sorted {
        let fair = remaining / left;
        if p > fair {
            return fair;
        }
        remaining -= p;
        left -= 1;
    }
    usize::MAX
}

/// Admits the best-ranked prefix whose cut docs each keep at least `MIN_SLICE_BYTES`.
fn admit(payloads: &[usize], budget: usize) -> (usize, usize) {
    let mut n = payloads.len();
    while n > 0 {
        let level = water_level(&payloads[..n], budget);
        let starved = level < MIN_SLICE_BYTES && payloads[..n].iter().any(|&p| p > level);
        if !starved {
            return (n, level);
        }
        n -= 1;
    }
    (0, 0)
}

fn truncate_lines(text: &str, limit: usize) -> (&str, usize) {
    let mut end = 0usize;
    let mut lines = 0usize;
    for line in text.split_inclusive('\n') {
        if end + line.len() > limit {
            break;
        }
        end += line.len();
        lines += 1;
    }
    if lines > 0 {
        return (&text[..end], lines);
    }
    let mut cut = limit.min(text.len());
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    (&text[..cut], 1)
}

fn base_doc(c: &Candidate<'_>) -> ServedDoc {
    ServedDoc {
        path: c.input.path.clone(),
        relevance: c.score,
        generated: c.generated,
        whole_bytes: c.input.text.len(),
        body: String::new(),
        sections: None,
        truncated: None,
    }
}

fn serve_headless(c: &Candidate<'_>, allowance: usize) -> ServedDoc {
    let text = &c.input.text;
    let mut doc = base_doc(c);
    if text.len() <= allowance {
        doc.body.clone_from(text);
        return doc;
    }
    let (part, _) = truncate_lines(text, allowance);
    doc.body = format!("{part}…");
    doc.truncated = Some((part.len(), text.len()));
    doc
}

/// The longest best-first run of matching sections that fits `allowance`, in document order.
fn pick_sections(c: &Candidate<'_>, allowance: usize) -> Vec<usize> {
    let mut order = c.hits.clone();
    order.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut used = 0usize;
    let mut picked: Vec<usize> = Vec::new();
    for (i, _) in order {
        used += c.sections[i].bytes();
        if used > allowance {
            break;
        }
        picked.push(i);
    }
    picked.sort_unstable();
    picked
}

fn serve_sections(c: &Candidate<'_>, allowance: usize) -> ServedDoc {
    let text = &c.input.text;
    let mut doc = base_doc(c);
    let picked = pick_sections(c, allowance);
    let mut spans = Vec::new();
    if picked.is_empty() {
        let top = c
            .hits
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
        let s = c.sections[top.map_or(0, |h| h.0)];
        let (part, lines) = truncate_lines(&text[s.start_byte..s.end_byte], allowance);
        doc.body = format!("{part}…");
        doc.truncated = Some((part.len(), s.bytes()));
        spans.push((s.start_line, s.start_line + lines - 1));
    } else {
        for &i in &picked {
            let s = c.sections[i];
            doc.body.push_str(&text[s.start_byte..s.end_byte]);
            spans.push((s.start_line, s.end_line));
        }
    }
    doc.sections = Some(SectionsNote {
        selected: spans.len(),
        matched: c.hits.len(),
        total: c.sections.len(),
        spans,
    });
    doc
}

#[must_use]
pub fn body_budget_bytes(max_tokens: usize) -> usize {
    (max_tokens as f64 * BODY_BYTES_PER_TOKEN).round() as usize
}

/// Ranks documents by BM25 and serves matching heading sections best-first under the token ceiling.
#[must_use]
pub fn recall(inputs: &[RecallInput], query: &str, opts: RecallOptions) -> RecallOutcome {
    let terms = query_terms(query);
    let ranked = if terms.is_empty() {
        Vec::new()
    } else {
        ranked_candidates(inputs, &terms)
    };
    let budget_bytes = body_budget_bytes(opts.max_tokens);
    let candidates = &ranked[..ranked.len().min(opts.top_k)];
    let payloads: Vec<usize> = candidates.iter().map(Candidate::payload).collect();
    let (admitted, level) = admit(&payloads, budget_bytes);
    let docs: Vec<ServedDoc> = candidates[..admitted]
        .iter()
        .zip(&payloads)
        .map(|(c, &p)| {
            let allowance = p.min(level);
            if c.hits.is_empty() {
                serve_headless(c, allowance)
            } else {
                serve_sections(c, allowance)
            }
        })
        .collect();
    let share_bytes = docs.iter().any(ServedDoc::is_cut).then_some(level);
    RecallOutcome {
        documents: inputs.len(),
        relevant: ranked.len(),
        generated_demoted: ranked.iter().filter(|c| c.generated).count(),
        omitted_top_k: ranked.len() - candidates.len(),
        omitted_budget: candidates.len() - admitted,
        top_k: opts.top_k,
        max_tokens: opts.max_tokens,
        budget_bytes,
        share_bytes,
        docs,
    }
}
