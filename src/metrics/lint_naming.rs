use super::lint_rules::{FileLintContext, FileSym, Sink};
use crate::domain::{CorpusView, IngestResult, Interner, SymKind};
use crate::languages::lint_hooks::declares_type;
use std::collections::HashMap;

const MAX_WORDS: usize = 5;
const MIN_CONFUSABLE_LEN: usize = 5;
const MAX_EXHAUSTIVE_NAMES: usize = 512;
const MAX_NAME_NEIGHBORHOOD: usize = 25;
const UNINFORMATIVE_MIN_LOC: u32 = 20;
const WIDTH_SUFFIXES: [&str; 8] = ["4", "6", "8", "16", "32", "64", "128", "256"];
const PREDICATE_PREFIXES: [&str; 5] = ["is", "has", "can", "should", "was"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Lower,
    Upper,
    Digit,
    Other,
}

fn class_of(c: char) -> CharClass {
    if c.is_lowercase() {
        CharClass::Lower
    } else if c.is_uppercase() {
        CharClass::Upper
    } else if c.is_ascii_digit() {
        CharClass::Digit
    } else {
        CharClass::Other
    }
}

fn word_boundary(prev: CharClass, current: CharClass, next: CharClass) -> bool {
    let digit_edge = (current == CharClass::Digit) != (prev == CharClass::Digit);
    let hump = prev == CharClass::Lower && current == CharClass::Upper;
    let acronym =
        prev == CharClass::Upper && current == CharClass::Upper && next == CharClass::Lower;
    digit_edge || hump || acronym
}

pub fn words(name: &str) -> Vec<&str> {
    let mut chars = name.char_indices().peekable();
    let mut out = Vec::new();
    let mut start = None;
    let mut prev = CharClass::Other;
    while let Some((i, c)) = chars.next() {
        let current = class_of(c);
        let next = chars.peek().map_or(CharClass::Other, |&(_, n)| class_of(n));
        if (current == CharClass::Other || word_boundary(prev, current, next))
            && let Some(s) = start.take()
        {
            out.push(&name[s..i]);
        }
        if current != CharClass::Other {
            start.get_or_insert(i);
        }
        prev = current;
    }
    if let Some(s) = start {
        out.push(&name[s..]);
    }
    out
}

#[derive(Debug, Default)]
pub struct NameCorpus {
    names: usize,
    df: HashMap<String, usize>,
}

impl NameCorpus {
    #[must_use]
    pub fn build(ingest: &IngestResult, interner: &Interner) -> Self {
        let view = (ingest, interner);
        let mut corpus = Self::default();
        for sym in ingest.symbols.iter().filter(|s| s.is_callable()) {
            corpus.names += 1;
            let mut seen: Vec<String> = words(view.sym_name(sym))
                .iter()
                .map(|w| w.to_ascii_lowercase())
                .collect();
            seen.sort_unstable();
            seen.dedup();
            for w in seen {
                *corpus.df.entry(w).or_default() += 1;
            }
        }
        corpus
    }

    fn is_ubiquitous(&self, word: &str) -> bool {
        self.df.get(word).is_some_and(|&n| n * 2 > self.names)
    }
}

fn is_naming_subject(ctx: &FileLintContext<'_>, s: &FileSym<'_>) -> bool {
    if s.name.is_empty() {
        return false;
    }
    match s.sym.kind {
        SymKind::Function | SymKind::Method => s.sym.end_byte > s.sym.sig_end_byte,
        SymKind::Var => {
            !ctx.in_callable(s.sym.sig_start_byte as usize)
                && !declares_type(ctx.declaration(s.sym), ctx.sem.type_declaration_keywords())
        }
        _ => false,
    }
}

fn subjects<'a>(ctx: &FileLintContext<'a>) -> impl Iterator<Item = FileSym<'a>> {
    ctx.syms
        .iter()
        .copied()
        .filter(|s| is_naming_subject(ctx, s))
}

fn callable_subjects<'a>(ctx: &FileLintContext<'a>) -> impl Iterator<Item = FileSym<'a>> {
    subjects(ctx).filter(|s| s.sym.is_callable())
}

/// Names of one or two letters once surrounding underscores are ignored.
#[must_use]
pub fn is_short_name(name: &str) -> bool {
    let core = name.trim_matches('_');
    !core.is_empty() && core.chars().count() <= 2 && core.chars().all(char::is_alphabetic)
}

/// Names split into more words than `MAX_WORDS`.
#[must_use]
pub fn is_wordy_name(name: &str) -> bool {
    words(name).len() > MAX_WORDS
}

pub fn short(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    for s in subjects(ctx) {
        if is_short_name(s.name) {
            let what = if s.sym.is_callable() {
                "1-2 letter function name; visible far beyond a 20-line scope"
            } else {
                "1-2 letter name for a module/global variable"
            };
            sink.at_symbol(s, format!("{} ({what})", s.name));
        }
    }
}

pub fn wordy(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    for s in subjects(ctx) {
        if is_wordy_name(s.name) {
            let n = words(s.name).len();
            sink.at_symbol(s, format!("{} ({n} words in one name)", s.name));
        }
    }
}

fn series_parts(name: &str) -> Option<(&str, &str)> {
    let base = name.trim_end_matches(|c: char| c.is_ascii_digit());
    let digits = &name[base.len()..];
    let member = !digits.is_empty() && base.chars().count() >= 2;
    member.then_some((base, digits))
}

pub fn series(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    let subs: Vec<_> = subjects(ctx).collect();
    let mut groups: HashMap<(Option<&str>, &str), Vec<&str>> = HashMap::new();
    for s in &subs {
        if let Some((base, _)) = series_parts(s.name) {
            let names = groups.entry((s.scope, base)).or_default();
            if !names.contains(&s.name) {
                names.push(s.name);
            }
        }
    }
    for s in &subs {
        let Some((base, _)) = series_parts(s.name) else {
            continue;
        };
        let names = &groups[&(s.scope, base)];
        let n = names.len();
        if n >= 2
            && names
                .iter()
                .any(|name| !WIDTH_SUFFIXES.contains(&&name[base.len()..]))
        {
            let msg = format!(
                "{} (number-series name; {n} siblings share the base '{base}')",
                s.name
            );
            sink.at_symbol(*s, msg);
        }
    }
}

pub fn underscore(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    for s in subjects(ctx) {
        if ctx.sem.is_reserved_identifier(s.name) {
            let what =
                "reserved identifier form: leading underscore + capital or double underscore";
            sink.at_symbol(s, format!("{} ({what})", s.name));
        } else if s.name.trim_matches('_').contains("__") {
            sink.at_symbol(
                s,
                format!("{} (consecutive underscores inside a name)", s.name),
            );
        }
    }
}

fn has_camel_hump(segment: &str) -> bool {
    let mut prev_lower = false;
    for c in segment.chars() {
        if prev_lower && c.is_uppercase() {
            return true;
        }
        prev_lower = c.is_lowercase();
    }
    false
}

pub fn mixed_case(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    for s in subjects(ctx) {
        let mut segments = s.name.split('_').filter(|p| !p.is_empty());
        let first = segments.next();
        let rest: Vec<&str> = segments.collect();
        let mixed = first.is_some()
            && !rest.is_empty()
            && (first.into_iter().chain(rest).any(has_camel_hump));
        if mixed {
            sink.at_symbol(
                s,
                format!("{} (snake_case and camelCase mixed in one name)", s.name),
            );
        }
    }
}

fn is_bare_identifier(ty: &str) -> bool {
    ty.chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
        && ty.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn has_name_prefix(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| c == '_' || c.is_uppercase())
}

fn declared_return_type<'a>(ctx: &FileLintContext<'a>, s: &FileSym<'_>) -> Option<&'a str> {
    ctx.sem
        .lint_return_type(ctx.signature(s.sym), s.name)
        .filter(|t| is_bare_identifier(t))
}

pub fn predicate(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    let bools = ctx.sem.lint_bool_types();
    for s in callable_subjects(ctx) {
        if PREDICATE_PREFIXES
            .iter()
            .any(|p| has_name_prefix(s.name, p))
            && declared_return_type(ctx, &s).is_some_and(|t| !bools.contains(&t))
        {
            let what = "predicate-named but the declared return type is not bool-like";
            sink.at_symbol(s, format!("{} ({what})", s.name));
        }
    }
}

pub fn setter(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    let voids = ctx.sem.lint_void_types();
    for s in callable_subjects(ctx) {
        if has_name_prefix(s.name, "set")
            && declared_return_type(ctx, &s).is_some_and(|t| !voids.contains(&t))
        {
            sink.at_symbol(
                s,
                format!(
                    "{} (set-prefixed but declares a non-void return type)",
                    s.name
                ),
            );
        }
    }
}

struct NameInfo<'a> {
    sym: FileSym<'a>,
    base: Option<&'a str>,
    key: String,
}

impl<'a> NameInfo<'a> {
    fn new(sym: FileSym<'a>) -> Self {
        let mut tokens: Vec<String> = words(sym.name).iter().map(|w| w.to_lowercase()).collect();
        tokens.sort_unstable();
        Self {
            sym,
            base: series_parts(sym.name).map(|(b, _)| b),
            key: tokens.join("_"),
        }
    }
}

fn edits_left(a: &str, b: &str, remaining: u8) -> bool {
    let shared = a
        .chars()
        .zip(b.chars())
        .take_while(|(x, y)| x == y)
        .map(|(c, _)| c.len_utf8())
        .sum::<usize>();
    let (a, b) = (&a[shared..], &b[shared..]);
    if a.is_empty() || b.is_empty() {
        return a.chars().count().max(b.chars().count()) <= usize::from(remaining);
    }
    if remaining == 0 {
        return false;
    }
    let aa = &a[a.chars().next().map_or(0, char::len_utf8)..];
    let bb = &b[b.chars().next().map_or(0, char::len_utf8)..];
    edits_left(aa, bb, remaining - 1)
        || edits_left(aa, b, remaining - 1)
        || edits_left(a, bb, remaining - 1)
}

fn within_two_edits(a: &str, b: &str) -> bool {
    a.chars().count().abs_diff(b.chars().count()) <= 2 && edits_left(a, b, 2)
}

fn is_digit_twin(a: &NameInfo<'_>, b: &NameInfo<'_>) -> bool {
    let twin = |series: &NameInfo<'_>, bare: &NameInfo<'_>| {
        series.base == Some(bare.sym.name) && bare.sym.name.chars().count() >= 2
    };
    twin(a, b) || twin(b, a)
}

fn confusion(a: &NameInfo<'_>, b: &NameInfo<'_>) -> Option<&'static str> {
    let (na, nb) = (a.sym.name, b.sym.name);
    if na == nb || a.sym.scope != b.sym.scope {
        return None;
    }
    if is_digit_twin(a, b) {
        return Some("differ only by a digit suffix");
    }
    if a.base.is_some() && a.base == b.base {
        return None;
    }
    if na.chars().count() < MIN_CONFUSABLE_LEN || nb.chars().count() < MIN_CONFUSABLE_LEN {
        return None;
    }
    if within_two_edits(na, nb) {
        return Some("edit distance <=2");
    }
    (a.key == b.key && words(na).len() > 1).then_some("same words in a different order")
}

fn confusable_pool(infos: &[NameInfo<'_>], sink: &mut Sink<'_>) {
    let capped = infos.len() > MAX_EXHAUSTIVE_NAMES;
    sink.capped |= capped;
    let neighborhood = if capped {
        MAX_NAME_NEIGHBORHOOD
    } else {
        infos.len()
    };
    // Bound each name's lexical neighborhood, not the file's source-order prefix.
    // Keep source ordering for the finding's message and location.
    for (i, first) in infos.iter().enumerate() {
        let end = infos.len().min(i + neighborhood);
        for second in &infos[i + 1..end] {
            let (earlier, later) = if first.sym.sym.sig_start_byte <= second.sym.sym.sig_start_byte
            {
                (first, second)
            } else {
                (second, first)
            };
            if let Some(reason) = confusion(earlier, later) {
                let msg = format!("{} ~ {} ({reason})", earlier.sym.name, later.sym.name);
                sink.at_symbol(later.sym, msg);
            }
        }
    }
}

pub fn confusable(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    let mut seen = std::collections::HashSet::new();
    let mut infos: Vec<NameInfo<'_>> = subjects(ctx)
        .map(|mut s| {
            s.scope = ctx.sem.lint_name_scope(s.scope);
            s
        })
        .filter(|s| seen.insert((s.scope, s.name)))
        .map(NameInfo::new)
        .collect();
    infos.sort_unstable_by_key(|info| (info.sym.scope, info.sym.name, info.sym.sym.sig_start_byte));
    for pool in infos.chunk_by(|a, b| a.sym.scope == b.sym.scope) {
        confusable_pool(pool, sink);
    }
}

pub fn uninformative(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    for s in callable_subjects(ctx).filter(|s| s.sym.loc >= UNINFORMATIVE_MIN_LOC) {
        let parts = words(s.name);
        if !parts.is_empty()
            && parts
                .iter()
                .all(|w| ctx.corpus.is_ubiquitous(&w.to_ascii_lowercase()))
        {
            let msg = format!("{} (every subtoken is ubiquitous in this corpus)", s.name);
            sink.at_symbol(s, msg);
        }
    }
}
