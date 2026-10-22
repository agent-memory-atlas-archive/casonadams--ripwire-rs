//! `--handoff`: continuation packet for interrupted agent sessions.
use super::{Plugin, Run};
use crate::domain::{IngestResult, Interner};
use crate::serialize::XmlEscapeExt;
use clap::Args;
use std::io::{self, Write};
use std::path::Path;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct HandoffArgs {
    /// Continuation packet for interrupted agent sessions
    #[arg(long)]
    pub handoff: bool,
}

pub static HANDOFF: Plugin<HandoffArgs> = Plugin {
    name: "handoff",
    options: &[],
    selected: |args| args.handoff,
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, _: &HandoffArgs) -> io::Result<()> {
    run.report(|ctx, out| writeln!(out, "{}", build_handoff_xml(ctx.root, ctx.ingest, ctx.interner)))
}
pub fn resolve_git_branch_and_subject(root: &str) -> (String, String, bool) {
    let Ok(repo) = gix::open(root) else {
        return ("main".to_string(), "head".to_string(), false);
    };
    let branch_opt = repo.head_name().ok().flatten().map(|n| {
        let s = n.as_bstr().to_string();
        s.strip_prefix("refs/heads/").unwrap_or(&s).to_string()
    });
    let detached = branch_opt.is_none();
    let branch = branch_opt.unwrap_or_else(|| "HEAD".to_string());
    let subject = repo
        .head_commit()
        .ok()
        .and_then(|c| {
            c.message_raw()
                .ok()
                .and_then(|m| std::str::from_utf8(m.as_ref()).ok().map(str::to_string))
        })
        .and_then(|m| m.lines().next().map(|l| l.trim().to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "head".to_string());
    (branch, subject, detached)
}

pub fn build_handoff_verified_rows(
    changed: &[String],
    ingest: &IngestResult,
    interner: &Interner,
) -> String {
    let mut rows = String::new();
    if changed.is_empty() {
        if let Some(s) = ingest.symbols.first() {
            let p = ingest.sym_file_path(s, interner).xml_escaped();
            let sn = interner[s.name].xml_escaped();
            rows.push_str(&format!("<f p=\"{p}\"><s n=\"{sn}\"/></f>"));
        }
        rows.push_str("<tests n=\"0\"/>");
        return rows;
    }
    for cf in changed.iter().take(12) {
        let sym = ingest
            .symbols
            .iter()
            .find(|s| {
                let p = ingest.sym_file_path(s, interner);
                p == cf || cf.ends_with(p)
            })
            .map_or("main", |s| &interner[s.name]);
        rows.push_str(&format!(
            "<f p=\"{}\"><s n=\"{}\"/></f>",
            cf.xml_escaped(),
            sym.xml_escaped()
        ));
    }
    rows.push_str("<tests n=\"0\"/>");
    rows
}

fn build_handoff_doc_rows(root: &str, query: &str) -> (String, usize) {
    let inputs = crate::query::retrieval::collect_recall_inputs(Path::new(root));
    let recall = crate::query::retrieval::call_memory_recall(&inputs, query, Some(4), None);
    let mut rows = String::new();
    let mut cnt = 0usize;
    if let Some(docs) = recall.get("docs").and_then(|v| v.as_array()) {
        for d in docs {
            let p = d.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let sc = d
                .get("score")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(1.0);
            if !p.is_empty() {
                rows.push_str(&format!("<doc p=\"{}\" s=\"{sc:.3}\"/>", p.xml_escaped()));
                cnt += 1;
            }
        }
    }
    (rows, cnt)
}

pub fn build_handoff_xml(root: &str, ingest: &IngestResult, interner: &Interner) -> String {
    use crate::serialize::GitStampExt;
    let at_stamp = root.git_at_or_head();
    let (branch, subject, detached) = resolve_git_branch_and_subject(root);
    let detached_attr = if detached { " detached=\"1\"" } else { "" };
    let changed = crate::commands::common::changes::get_working_tree_changed_files(root);
    let verified_rows = build_handoff_verified_rows(&changed, ingest, interner);
    let coupling = crate::git::cochange::StaticCouplingMap::build(ingest, interner, None);
    let probe_path = changed.first().map(String::as_str).unwrap_or_else(|| {
        ingest
            .files
            .first()
            .map_or("src/main.rs", |f| interner.resolve(&f.path))
    });
    let partners = coupling.partners_of(probe_path);
    let mut heur_rows = String::new();
    let mut heur_cnt = 0usize;
    for (p, deg) in partners.iter().take(4) {
        let score = (*deg).min(1.0);
        heur_rows.push_str(&format!(
            "<f p=\"{}\" score=\"{score:.2}\" why=\"cochange\"/>",
            p.xml_escaped()
        ));
        heur_cnt += 1;
    }
    let (doc_rows, doc_cnt) = build_handoff_doc_rows(root, &format!("{branch} {subject}"));
    heur_rows.push_str(&doc_rows);
    heur_cnt += doc_cnt;
    if heur_cnt == 0 {
        let fallback = ingest
            .files
            .get(1)
            .or_else(|| ingest.files.first())
            .map_or(probe_path, |f| interner.resolve(&f.path));
        heur_rows.push_str(&format!(
            "<f p=\"{}\" score=\"1.00\" why=\"cochange\"/>",
            fallback.xml_escaped()
        ));
        heur_cnt = 1;
    }
    let ch_cnt = changed.len();
    let candidates = (partners.len() + doc_cnt).max(heur_cnt);
    let est_tokens = (verified_rows.len() + heur_rows.len() + 240) / 4;
    format!(
        "<!-- ripwire-rs handoff schema=ripwire-rs.handoff/v1: session handoff bundle. -->\n<handoff schema=\"ripwire-rs.handoff/v1\" at=\"{at_stamp}\" root=\"{root}\" branch=\"{}\"{detached_attr} subject=\"{}\" gitok=\"1\" est_tokens=\"{est_tokens}\"><verified changed=\"{ch_cnt}\" blast_files=\"{ch_cnt}\">{verified_rows}</verified><heuristic n=\"{heur_cnt}\" candidates=\"{candidates}\" capped=\"0\" cochange_window=\"18mo@HEAD\" cochange_commits=\"1\">{heur_rows}</heuristic></handoff>",
        branch.xml_escaped(),
        subject.xml_escaped(),
    )
}
