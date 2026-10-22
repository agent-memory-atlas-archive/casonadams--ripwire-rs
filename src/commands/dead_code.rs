//! `--dead-code`: internal functions with zero incoming callers; honors `--limit` / `--offset`.
use super::{Plugin, Run, SharedOption};
use crate::commands::context::ExecutionContext;
use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::serialize::XmlWriter;
use clap::Args;
use std::io::{self, Write};
use std::path::Path;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct DeadCodeArgs {
    /// Internal functions with zero incoming callers across the repository
    #[arg(long = "dead-code")]
    pub dead_code: bool,
}

struct CachedFile<'a> {
    source: std::borrow::Cow<'a, [u8]>,
    tree: Option<tree_sitter::Tree>,
}

fn parse_file_tree(
    lang: crate::domain::Lang,
    path: &str,
    source: &[u8],
) -> Option<tree_sitter::Tree> {
    let grammar = lang.semantics().source_grammar(path)?;
    crate::ingest::registry::PARSER_STATE.with(|state| {
        let mut borrowed = state.borrow_mut();
        let (parser, _) = &mut *borrowed;
        parser.set_language(grammar).ok()?;
        parser.parse(source, None)
    })
}

pub struct DeadCodeContext<'a> {
    pub root: &'a str,
    pub sources: &'a dyn crate::query::SourceReader,
    pub ingest: &'a IngestResult,
    pub graph: &'a Graph,
    pub interner: &'a Interner,
    pub page: &'a dyn crate::commands::context::Paginated,
}

pub fn execute_dead_code_query_paged<W: Write>(
    ctx: &DeadCodeContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    use crate::domain::CorpusView;
    let view = (ctx.ingest, ctx.interner);
    let mut file_cache: std::collections::HashMap<u32, CachedFile<'_>> =
        std::collections::HashMap::new();
    let mut dead = Vec::new();
    for sym in &ctx.ingest.symbols {
        if sym.kind != crate::domain::SymKind::Function
            || !sym.lang.semantics().supports_internal_linkage()
            || sym.sig_end_byte >= sym.end_byte
            || ctx.graph.in_degree(sym.id) != 0
        {
            continue;
        }
        let p = view.sym_path(sym);
        if sym.lang.semantics().is_header_file(p) {
            continue;
        }
        let cached = file_cache.entry(sym.file_id).or_insert_with(|| {
            let source = ctx
                .sources
                .read(p)
                .unwrap_or(std::borrow::Cow::Borrowed(&[]));
            let tree = parse_file_tree(sym.lang, p, source.as_ref());
            CachedFile { source, tree }
        });
        let is_internal = cached.tree.as_ref().is_some_and(|tree| {
            sym.lang.semantics().is_internal_function(
                tree.root_node(),
                sym.sig_start_byte as usize,
                cached.source.as_ref(),
            )
        });
        if is_internal {
            dead.push((view.sym_name(sym), sym.kind.tag(), p, sym.line));
        }
    }
    dead.sort_by(|a, b| {
        a.2.cmp(b.2)
            .then_with(|| a.3.cmp(&b.3))
            .then_with(|| a.0.cmp(b.0))
    });
    crate::serialize::query_xml::serialize_dead_code_paged_xml(
        ctx.root,
        &dead,
        ctx.graph.gauges(),
        ctx.page.page_request(),
        out,
    )
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &DeadCodeArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    execute_dead_code_query_paged(
        &DeadCodeContext {
            root: ctx.root,
            sources: &crate::query::FileSources {
                root: Path::new(ctx.root),
            },
            ingest: ctx.ingest,
            graph: ctx.graph,
            interner: ctx.interner,
            page: ctx.cli,
        },
        out,
    )
}

fn run(run: &Run<'_>, args: &DeadCodeArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub static DEAD_CODE: Plugin<DeadCodeArgs> = Plugin {
    name: "dead-code",
    options: &[SharedOption::Paged],
    selected: |args| args.dead_code,
    run,
    check: None,
    tools: &[],
};
