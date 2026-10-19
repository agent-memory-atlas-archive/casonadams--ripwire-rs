use super::{find_symbols_by_name, transitive_impact_multi};
use crate::domain::{CorpusView, IngestResult, Interner, NodeId};
use crate::graph::Graph;
use crate::serialize::{GraphGauges, PathLayerExt, XmlEscapeExt, XmlReport};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Write};

fn reconstruct_path(mut node: NodeId, parent: &HashMap<NodeId, NodeId>) -> Vec<NodeId> {
    let mut path = vec![node];
    while let Some(&prev) = parent.get(&node) {
        if prev == node {
            break;
        }
        path.push(prev);
        node = prev;
    }
    path.reverse();
    path
}

fn bfs_shortest_path(graph: &Graph, from_ids: &[NodeId], to_ids: &[NodeId]) -> Vec<NodeId> {
    if let ([s], [t]) = (from_ids, to_ids)
        && s == t
    {
        return vec![*s];
    }
    let targets: HashSet<NodeId> = to_ids.iter().copied().collect();
    let mut queue: VecDeque<NodeId> = from_ids.iter().copied().collect();
    let mut parent: HashMap<NodeId, NodeId> = from_ids.iter().map(|&id| (id, id)).collect();
    while let Some(curr) = queue.pop_front() {
        for (callee, _) in graph.callees(curr) {
            if let std::collections::hash_map::Entry::Vacant(e) = parent.entry(callee) {
                e.insert(curr);
                if targets.contains(&callee) {
                    return reconstruct_path(callee, &parent);
                }
                queue.push_back(callee);
            }
        }
    }
    Vec::new()
}

fn write_sym_witness<W: Write>(
    id: NodeId,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    if let Some(sym) = ingest.sym(id) {
        let name = interner.resolve(&sym.name).xml_escaped();
        let path = ingest.sym_file_path(sym, interner).xml_escaped();
        write!(
            out,
            "<s t=\"{}\" n=\"{name}\" p=\"{path}:{}\"/>",
            sym.kind.tag(),
            sym.line
        )?;
    }
    Ok(())
}

fn collect_literal_hits(
    file_pat: &str,
    lit: &str,
    ingest: &IngestResult,
    interner: &Interner,
) -> Vec<(String, u32, String, String)> {
    let val = crate::mcp::retrieval::call_grep(ingest, interner, lit, Some(file_pat));
    let mut hits = Vec::new();
    if let Some(arr) = val["hits"].as_array() {
        for h in arr {
            hits.push((
                h["path"].as_str().unwrap_or("").to_string(),
                h["line"].as_u64().unwrap_or(1) as u32,
                h["enclosing"].as_str().unwrap_or("").to_string(),
                h["text"].as_str().unwrap_or("").to_string(),
            ));
        }
    }
    if !hits.is_empty() {
        return hits;
    }
    for sym in &ingest.symbols {
        let path = ingest.sym_file_path(sym, interner);
        let name = interner.resolve(&sym.name);
        if path.contains(file_pat) && name.contains(lit) {
            hits.push((
                path.to_string(),
                sym.line,
                name.to_string(),
                name.to_string(),
            ));
        }
    }
    hits
}

fn write_hit_witness<W: Write>(
    (path, line, enc, text): &(String, u32, String, String),
    out: &mut W,
) -> io::Result<()> {
    let safe = text.replace("]]>", "]]]]><![CDATA[>");
    write!(
        out,
        "<hit p=\"{}:{line}\" in=\"{}\"><m><![CDATA[{safe}]]></m></hit>",
        path.xml_escaped(),
        enc.xml_escaped()
    )
}

pub enum VerifyWitness<'a> {
    Nodes(Vec<NodeId>),
    Uses(Vec<(&'static str, &'a str, u32, Option<&'a str>)>),
    Hits(Vec<(String, u32, String, String)>),
}

impl VerifyWitness<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Nodes(v) => v.len(),
            Self::Uses(v) => v.len(),
            Self::Hits(v) => v.len(),
        }
    }
}

pub struct VerifyReport<'a> {
    pub claim: &'a str,
    pub shape: &'static str,
    pub verdict: &'static str,
    pub facts: String,
    pub limit: Option<&'static str>,
    pub complete: bool,
    pub witness: VerifyWitness<'a>,
    pub ingest: &'a IngestResult,
    pub interner: &'a Interner,
    pub gauges: GraphGauges,
}

impl XmlReport for VerifyReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "verify"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire-rs.verify/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire-rs verify schema=ripwire-rs.verify/v1: ONE structured claim, verdict=confirmed|refuted|not-established, witness rows inline.",
        )
    }
    fn graph_gauges(&self) -> Option<GraphGauges> {
        (!self.complete).then_some(self.gauges)
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let total = self.witness.len();
        let shown = total.min(20);
        let capped = usize::from(shown < total);
        write!(
            out,
            " claim=\"{}\" shape=\"{}\" verdict=\"{}\"{}",
            self.claim.xml_escaped(),
            self.shape,
            self.verdict,
            self.facts
        )?;
        if let Some(lim) = self.limit {
            write!(out, " limit=\"{lim}\"")?;
        }
        if self.complete {
            write!(out, " complete=\"1\"")?;
        }
        write!(out, " shown=\"{shown}\" capped=\"{capped}\"")
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        match &self.witness {
            VerifyWitness::Nodes(ids) => {
                for &id in ids.iter().take(20) {
                    write_sym_witness(id, self.ingest, self.interner, out)?;
                }
            }
            VerifyWitness::Uses(sites) => {
                for &(role, file, line, enc) in sites.iter().take(20) {
                    let f_esc = file.xml_escaped();
                    match enc.filter(|s| !s.is_empty()) {
                        Some(e) => write!(
                            out,
                            "<u role=\"{role}\" p=\"{f_esc}:{line}\" in_id=\"{}\"/>",
                            e.xml_escaped()
                        )?,
                        None => write!(out, "<u role=\"{role}\" p=\"{f_esc}:{line}\"/>")?,
                    }
                }
            }
            VerifyWitness::Hits(hits) => {
                for h in hits.iter().take(20) {
                    write_hit_witness(h, out)?;
                }
            }
        }
        Ok(())
    }
}

fn parse_verify_claim(claim: &str) -> Option<(&str, &str, &str, bool)> {
    let trimmed = claim.trim();
    let open = trimmed.find('(')?;
    if !trimmed.ends_with(')') {
        return None;
    }
    let word = trimmed[..open].trim();
    let inner = trimmed[open + 1..trimmed.len() - 1].trim();
    if matches!(word, "uses" | "unused") {
        return (!inner.is_empty() && !inner.contains(',')).then_some((word, inner, "", false));
    }
    let (a1, a2) = inner.split_once(',')?;
    let (arg1, rest) = (a1.trim(), a2.trim());
    if arg1.is_empty() || rest.is_empty() {
        return None;
    }
    let quoted = (rest.starts_with('"') && rest.ends_with('"')
        || rest.starts_with('\'') && rest.ends_with('\''))
        && rest.len() >= 2;
    Some((
        word,
        arg1,
        if quoted {
            &rest[1..rest.len() - 1]
        } else {
            rest
        },
        quoted,
    ))
}

fn eval_calls<'a>(
    claim: &'a str,
    a: &str,
    b: &str,
    ingest: &'a IngestResult,
    graph: &Graph,
    interner: &'a Interner,
) -> VerifyReport<'a> {
    let from_ids = find_symbols_by_name(a, ingest, interner);
    let to_ids = find_symbols_by_name(b, ingest, interner);
    let path = bfs_shortest_path(graph, &from_ids, &to_ids);
    let ok = !path.is_empty();
    let hops = if ok {
        format!(" hops=\"{}\"", path.len() - 1)
    } else {
        String::new()
    };
    VerifyReport {
        claim,
        shape: "calls",
        verdict: if ok { "confirmed" } else { "not-established" },
        facts: format!(
            " from_defs=\"{}\" to_defs=\"{}\"{hops}",
            from_ids.len(),
            to_ids.len()
        ),
        limit: (!ok).then_some("call-graph-floor"),
        complete: false,
        witness: VerifyWitness::Nodes(path),
        ingest,
        interner,
        gauges: graph.gauges(),
    }
}

fn eval_uses<'a>(
    claim: &'a str,
    sym: &str,
    is_unused: bool,
    ingest: &'a IngestResult,
    graph: &Graph,
    interner: &'a Interner,
) -> VerifyReport<'a> {
    let defs = find_symbols_by_name(sym, ingest, interner);
    let sites = (ingest, interner).collect_symbol_uses(sym);
    let total = sites.len();
    let verdict = match (is_unused, total > 0) {
        (false, true) => "confirmed",
        (true, true) => "refuted",
        _ => "not-established",
    };
    VerifyReport {
        claim,
        shape: if is_unused { "unused" } else { "uses" },
        verdict,
        facts: format!(
            " defs=\"{}\" external=\"{}\" count=\"{total}\"",
            defs.len(),
            usize::from(defs.is_empty())
        ),
        limit: (total == 0).then_some("reference-floor"),
        complete: false,
        witness: VerifyWitness::Uses(sites),
        ingest,
        interner,
        gauges: graph.gauges(),
    }
}

fn eval_defines<'a>(
    claim: &'a str,
    file_pat: &str,
    sym: &str,
    ingest: &'a IngestResult,
    graph: &Graph,
    interner: &'a Interner,
) -> VerifyReport<'a> {
    let defs_of_name = find_symbols_by_name(sym, ingest, interner);
    let in_file: Vec<NodeId> = defs_of_name
        .iter()
        .copied()
        .filter(|&id| {
            ingest
                .sym(id)
                .is_some_and(|s| ingest.sym_file_path(s, interner).contains(file_pat))
        })
        .collect();
    if !in_file.is_empty() {
        return VerifyReport {
            claim,
            shape: "defines",
            verdict: "confirmed",
            facts: format!(
                " defs=\"{}\" defs_of_name=\"{}\"",
                in_file.len(),
                defs_of_name.len()
            ),
            limit: None,
            complete: false,
            witness: VerifyWitness::Nodes(in_file),
            ingest,
            interner,
            gauges: graph.gauges(),
        };
    }
    let hits = collect_literal_hits(file_pat, sym, ingest, interner);
    let occ = hits.len();
    VerifyReport {
        claim,
        shape: "defines",
        verdict: if occ > 0 {
            "not-established"
        } else {
            "refuted"
        },
        facts: format!(
            " defs=\"0\" defs_of_name=\"{}\" occurrences=\"{occ}\"",
            defs_of_name.len()
        ),
        limit: (occ > 0).then_some("extraction-floor"),
        complete: occ == 0,
        witness: VerifyWitness::Hits(hits),
        ingest,
        interner,
        gauges: graph.gauges(),
    }
}

fn eval_reaches<'a>(
    claim: &'a str,
    sym: &str,
    target: &str,
    is_quoted: bool,
    ingest: &'a IngestResult,
    graph: &Graph,
    interner: &'a Interner,
) -> VerifyReport<'a> {
    let target_defs = find_symbols_by_name(sym, ingest, interner);
    let ranks = vec![0.0; ingest.symbols.len()];
    let impact = transitive_impact_multi(&target_defs, ingest, graph, &ranks, interner, None);
    let mut nodes = Vec::new();
    let mut first_hops = None;
    for node in &impact {
        let matched = if is_quoted {
            node.path.contains(target)
        } else {
            node.path
                .builtin_layer()
                .is_some_and(|l| l.eq_ignore_ascii_case(target))
        };
        if matched {
            first_hops.get_or_insert(node.hops);
            nodes.push(node.id);
        }
    }
    let hops_attr = first_hops.map_or_else(String::new, |h| format!(" hops=\"{h}\""));
    VerifyReport {
        claim,
        shape: "reaches",
        verdict: if nodes.is_empty() {
            "not-established"
        } else {
            "confirmed"
        },
        facts: format!(
            " target_defs=\"{}\" witnesses=\"{}\"{hops_attr}",
            target_defs.len(),
            nodes.len()
        ),
        limit: nodes.is_empty().then_some("call-graph-floor"),
        complete: false,
        witness: VerifyWitness::Nodes(nodes),
        ingest,
        interner,
        gauges: graph.gauges(),
    }
}

pub fn evaluate_verify_claim<W: Write>(
    claim: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let rep = match parse_verify_claim(claim) {
        Some(("calls", a, b, _)) => eval_calls(claim, a, b, ingest, graph, interner),
        Some(("uses", sym, _, _)) => eval_uses(claim, sym, false, ingest, graph, interner),
        Some(("unused", sym, _, _)) => eval_uses(claim, sym, true, ingest, graph, interner),
        Some(("contains", f, lit, _)) => {
            let hits = collect_literal_hits(f, lit, ingest, interner);
            let total = hits.len();
            VerifyReport {
                claim,
                shape: "contains",
                verdict: if total > 0 { "confirmed" } else { "refuted" },
                facts: format!(" hits=\"{total}\""),
                limit: None,
                complete: total <= 20,
                witness: VerifyWitness::Hits(hits),
                ingest,
                interner,
                gauges: graph.gauges(),
            }
        }
        Some(("defines", f, sym, _)) => eval_defines(claim, f, sym, ingest, graph, interner),
        Some(("reaches", sym, target, q)) => {
            eval_reaches(claim, sym, target, q, ingest, graph, interner)
        }
        _ => VerifyReport {
            claim,
            shape: "calls",
            verdict: "not-established",
            facts: " from_defs=\"0\" to_defs=\"0\"".to_string(),
            limit: Some("call-graph-floor"),
            complete: false,
            witness: VerifyWitness::Nodes(Vec::new()),
            ingest,
            interner,
            gauges: graph.gauges(),
        },
    };
    rep.render_envelope(out)
}
