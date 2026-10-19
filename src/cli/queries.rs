pub use super::arch::*;
pub use super::detail::*;
pub use super::retrieval::*;
pub use super::task_route::*;
pub use super::workspace::*;

use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::rank::PageRankResult;
use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavMode {
    Callers,
    Callees,
    Impact,
    Uses,
    FindSymbol,
}

impl NavMode {
    #[must_use]
    pub const fn cli_flag(self) -> &'static str {
        match self {
            Self::Callers => "--callers",
            Self::Callees => "--callees",
            Self::Impact => "--impact",
            Self::Uses => "--uses",
            Self::FindSymbol => "--find-symbol",
        }
    }

    #[must_use]
    pub fn from_batch_verb(verb: &str) -> Option<Self> {
        match verb {
            "callers" => Some(Self::Callers),
            "callees" => Some(Self::Callees),
            "impact" => Some(Self::Impact),
            "uses" => Some(Self::Uses),
            "find_symbol" | "find-symbol" => Some(Self::FindSymbol),
            _ => None,
        }
    }
}

impl std::fmt::Display for NavMode {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.cli_flag())
    }
}

impl std::str::FromStr for NavMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_batch_verb(s).ok_or_else(|| format!("unknown navigation mode: '{s}'"))
    }
}
pub struct NavContext<'a> {
    pub ingest: &'a IngestResult,
    pub graph: &'a Graph,
    pub ranks: &'a [f64],
    pub interner: &'a Interner,
}

impl<'a> crate::domain::CorpusView<'a> for NavContext<'a> {
    #[inline]
    fn ingest(&self) -> &'a IngestResult {
        self.ingest
    }

    #[inline]
    fn interner(&self) -> &'a Interner {
        self.interner
    }
}

pub fn serialize_1hop_mode<W: Write>(
    root: &str,
    sym: &str,
    mode: NavMode,
    nav: &crate::navigate::NavNeighborhood,
    out: &mut W,
) -> io::Result<()> {
    match mode {
        NavMode::Callers => crate::navigate::serialize_callers_with_gauge_xml(
            root,
            sym,
            &nav.callers,
            &nav.gauge_attrs,
            out,
        ),
        NavMode::Callees => crate::navigate::serialize_callees_with_gauge_xml(
            root,
            sym,
            &nav.callees,
            &nav.gauge_attrs,
            out,
        ),
        NavMode::FindSymbol => crate::navigate::serialize_find_symbol_xml(nav, out),
        _ => Ok(()),
    }
}

pub fn execute_nav_symbol_query<W: Write>(
    sym: &str,
    mode: NavMode,
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let ctx = NavContext {
        ingest,
        graph,
        ranks,
        interner,
    };
    execute_nav_symbol_query_in_root(".", sym, mode, &ctx, out)
}

pub fn execute_nav_symbol_query_in_root<W: Write>(
    root: &str,
    sym: &str,
    mode: NavMode,
    ctx: &NavContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    let ids = crate::navigate::find_symbols_by_name(sym, ctx.ingest, ctx.interner);
    if ids.is_empty() {
        let flag = mode.cli_flag();
        if let Some(near) = crate::navigate::find_nearest_symbol_name(sym, ctx.ingest, ctx.interner)
        {
            return Err(io::Error::other(format!(
                "ripwire-rs: {flag} symbol not found: {sym} (did you mean '{near}'?)"
            )));
        }
        return Err(io::Error::other(format!(
            "ripwire-rs: {flag} symbol not found: {sym}"
        )));
    }
    let gauge_attrs = ctx.graph.xml_gauge_attrs();
    match mode {
        NavMode::Uses => crate::navigate::serialize_uses_with_gauge_xml(
            root,
            sym,
            ctx.ingest,
            ctx.interner,
            &gauge_attrs,
            out,
        ),
        NavMode::Impact => {
            use crate::domain::CorpusView;
            let impact = crate::navigate::transitive_impact_multi(
                &ids,
                ctx.ingest,
                ctx.graph,
                ctx.ranks,
                ctx.interner,
                None,
            );
            let importers = (ctx.ingest, ctx.interner).resolve_symbol_importers(&ids);
            let pr_iters = ctx
                .graph
                .page_rank(&crate::rank::PageRankConfig::default())
                .iterations;
            crate::navigate::serialize_impact_with_importers_xml(
                root,
                sym,
                &impact,
                importers,
                &gauge_attrs,
                pr_iters,
                out,
            )
        }
        NavMode::Callers | NavMode::Callees | NavMode::FindSymbol => {
            if let Some(nav) = crate::navigate::navigate_1hop_multi(
                &ids,
                ctx.ingest,
                ctx.graph,
                ctx.ranks,
                ctx.interner,
            ) {
                serialize_1hop_mode(root, sym, mode, &nav, out)?;
            }
            Ok(())
        }
    }
}

pub fn execute_connect_query<W: Write>(
    syms: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    execute_connect_query_with_radius(".", syms, 6, ingest, graph, interner, out)
}

pub fn execute_connect_query_with_radius<W: Write>(
    root: &str,
    syms: &str,
    max_radius: u32,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let sym_list: Vec<String> = syms
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    for s in &sym_list {
        if crate::navigate::find_symbols_by_name(s, ingest, interner).is_empty() {
            return Err(io::Error::other(format!(
                "ripwire-rs: --connect symbol not found: {s}"
            )));
        }
    }
    let mut val = crate::mcp::graph_tools::call_connect_with_radius(
        ingest, graph, interner, &sym_list, max_radius,
    );
    val["root"] = serde_json::Value::String(root.to_string());
    crate::serialize::serialize_connect_xml(&val, out)
}

pub fn split_endpoint_pair(pair: &str) -> (&str, &str) {
    if let Some((from, to)) = pair.split_once(',') {
        (from.trim(), to.trim())
    } else if let Some((from, to)) = pair.split_once("->") {
        (from.trim(), to.trim())
    } else if let Some((from, to)) = pair.rsplit_once(':')
        && !from.ends_with(':')
    {
        (from.trim(), to.trim())
    } else if let Some((from, to)) = pair.split_once(':') {
        (from.trim(), to.trim())
    } else {
        (pair.trim(), "")
    }
}

pub fn execute_path_query<W: Write>(
    pair: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (from, to) = split_endpoint_pair(pair);
    if crate::navigate::find_symbols_by_name(from, ingest, interner).is_empty() {
        return Err(io::Error::other(format!(
            "ripwire-rs: --path endpoint not found: {from}"
        )));
    }
    if crate::navigate::find_symbols_by_name(to, ingest, interner).is_empty() {
        return Err(io::Error::other(format!(
            "ripwire-rs: --path endpoint not found: {to}"
        )));
    }
    let val = crate::mcp::graph_tools::call_path_between(ingest, graph, interner, from, to);
    crate::serialize::serialize_path_between_xml(&val, out)
}

pub fn deduplicate_batch_legend(raw: &[u8], legend_emitted: &mut bool) -> Vec<u8> {
    let text = String::from_utf8_lossy(raw);
    if !text.contains("<!--") {
        return raw.to_vec();
    }
    if !*legend_emitted {
        *legend_emitted = true;
        return raw.to_vec();
    }
    let mut out = String::with_capacity(text.len());
    let mut s: &str = &text;
    while let Some(start) = s.find("<!--") {
        out.push_str(&s[..start]);
        if let Some(end) = s[start..].find("-->") {
            s = &s[start + end + 3..];
        } else {
            break;
        }
    }
    out.push_str(s.trim_start());
    let trimmed = out.trim_end();
    format!(
        "{trimmed}\n<about legend=\"ref\" dictv=\"{}\"/>\n",
        crate::serialize::LEGEND_DICT_VERSION
    )
    .into_bytes()
}

fn run_batch_subquery_dedup<W: Write>(
    line: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    legend_emitted: &mut bool,
    out: &mut W,
) -> io::Result<()> {
    let Some((verb, arg)) = line.split_once(':') else {
        return Ok(());
    };
    let a = arg.trim();
    let mut sub_buf = Vec::new();
    let verb_trimmed = verb.trim();
    if let Ok(mode) = verb_trimmed.parse::<NavMode>() {
        let _ = execute_nav_symbol_query(a, mode, ingest, graph, &pr.ranks, interner, &mut sub_buf);
    } else if verb_trimmed == "grep" {
        let val = crate::mcp::retrieval::call_grep(ingest, interner, a, None);
        let _ = crate::serialize::serialize_grep_xml(&val, &mut sub_buf);
    } else if verb_trimmed == "for" {
        let val = crate::mcp::retrieval::call_for_task_with_options(
            ingest,
            interner,
            &pr.ranks,
            a,
            crate::mcp::retrieval::ForTaskOptions {
                root: ".",
                graph: Some(graph),
                ..Default::default()
            },
        );
        let _ = crate::serialize::serialize_for_xml(&val, &mut sub_buf);
    }
    let final_bytes = deduplicate_batch_legend(&sub_buf, legend_emitted);
    writeln!(out, "  <result query=\"{line}\">")?;
    out.write_all(&final_bytes)?;
    writeln!(out, "  </result>")
}

pub fn execute_batch_content<W: Write>(
    root: &str,
    content: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let lines: Vec<&str> = content
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .take(16)
        .collect();

    writeln!(
        out,
        "<batch schema=\"ripwire-rs.batch/v1\" root=\"{root}\" dictv=\"{}\" count=\"{}\">",
        crate::serialize::LEGEND_DICT_VERSION,
        lines.len()
    )?;
    let mut legend_emitted = false;
    for line in lines {
        run_batch_subquery_dedup(line, ingest, graph, pr, interner, &mut legend_emitted, out)?;
    }
    writeln!(out, "</batch>")
}
pub fn execute_batch_file<W: Write>(
    root: &str,
    file_path: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let content = if file_path == "-" {
        let mut buf = String::new();
        io::Read::read_to_string(&mut io::stdin(), &mut buf)?;
        buf
    } else {
        std::fs::read_to_string(file_path)?
    };
    execute_batch_content(root, &content, ingest, graph, pr, interner, out)
}
