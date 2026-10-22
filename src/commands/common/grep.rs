//! Corpus search shared by `--grep` and `--regex`, plus the modifiers they take; `--pattern` and `--match` reuse
//! [`write_hits`] for their paged hit lists.
use crate::commands::context::ExecutionContext;
use crate::serialize::PageRequest;
use clap::Args;
use serde_json::Value;
use std::io::{self, Write};

/// Modifiers of `--grep` / `--regex`: span tier, scope, extra terms and context lines.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct GrepModifiers {
    /// Span tier a --grep hit must sit in: code (default) or any
    #[arg(long = "grep-in", value_name = "code|any")]
    pub grep_in: Option<String>,
    /// Where --and/--not terms must appear: line (default) or file
    #[arg(long = "grep-scope", value_name = "line|file")]
    pub grep_scope: Option<String>,
    /// Keep only --grep hits where PAT is also present (repeatable)
    #[arg(long = "and", value_name = "PAT")]
    pub and_pat: Vec<String>,
    /// Drop --grep hits where PAT is present (repeatable)
    #[arg(long = "not", value_name = "PAT")]
    pub not_pat: Vec<String>,
    /// Lines before grep hit
    #[arg(long = "grep-before", value_name = "N")]
    pub grep_before: Option<usize>,
    /// Lines after grep hit
    #[arg(long = "grep-after", value_name = "N")]
    pub grep_after: Option<usize>,
    /// Lines around grep hit
    #[arg(long = "grep-context", value_name = "N")]
    pub grep_context: Option<usize>,
}

/// Searches the corpus for `pat` as `verb` (`grep` or `regex`) and writes the hits windowed by `page`.
pub fn execute<W: Write>(
    ctx: &ExecutionContext<'_>,
    verb: &str,
    pat: &str,
    page: PageRequest,
    in_scope: Option<&str>,
    modifiers: &GrepModifiers,
    out: &mut W,
) -> io::Result<()> {
    let sources = crate::query::FileSources {
        root: std::path::Path::new(ctx.root),
    };
    let corpus = crate::crawler::walk_corpus(ctx.root, &crate::crawler::CrawlerOptions::default());
    let req = crate::query::retrieval::GrepRequest {
        corpus_paths: &corpus.rel_paths,
        pruned_dirs: corpus.pruned_dirs,
        limit: page.limit,
        offset: page.offset.unwrap_or(0),
        verb,
        path_filter: in_scope,
        any_tier: crate::query::grep::parse_any_tier(modifiers.grep_in.as_deref())?,
        before: modifiers
            .grep_before
            .or(modifiers.grep_context)
            .unwrap_or(0),
        after: modifiers.grep_after.or(modifiers.grep_context).unwrap_or(0),
        and_terms: &modifiers.and_pat,
        not_terms: &modifiers.not_pat,
        file_scope: modifiers.grep_scope.as_deref() == Some("file"),
        graph: Some(ctx.graph),
        ..crate::query::retrieval::GrepRequest::new(ctx.root, pat, &sources)
    };
    let val = crate::query::retrieval::call_grep(ctx.ingest, ctx.interner, &req);
    crate::serialize::serialize_grep_xml(&val, out)
}

/// Writes grep-shaped hits, recording the `--limit` / `--offset` window so the report pages them.
pub fn write_hits<W: Write>(mut val: Value, page: PageRequest, out: &mut W) -> io::Result<()> {
    if let Some(limit) = page.limit {
        val["limit"] = Value::from(limit as u64);
    }
    if let Some(offset) = page.offset {
        val["offset"] = Value::from(offset as u64);
    }
    crate::serialize::serialize_grep_xml(&val, out)
}
