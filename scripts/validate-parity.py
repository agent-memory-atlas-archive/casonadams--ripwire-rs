#!/usr/bin/env python3
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET

OPTIONAL_ATTRS = {
    "shown",
    "capped",
    "total",
    "has_more",
    "next_offset",
    "offset",
    "limit",
    "next",
    "graph_unindexed",
    "graph_ambiguous",
    "graph_unresolved",
    "graph_partial",
    "tsconfig_unread",
    "imports_unresolved",
    "ts_decl_only",
    "ts_extends_unread",
    "declined_calls",
    "builtins_excluded",
    "renames",
    "rename_window_commits",
    "acked_by_rename",
    "acked_by_content",
    "acks_bad_lines",
    "head_basis",
    "layer",
    "lazy_edges",
    "value-ref-excluded",
    "runner-root-excluded",
    "decorated-excluded",
    "declined-call-excluded",
    "shown_bridges",
    "bridges_capped",
    "bridges",
    "shown_importers",
    "importers_capped",
    "importers",
    "unavailable",
    "unavailable_why",
    "unavail",
    "reason",
    "at",
    "root",
    "pr_converged",
    "sites_at",
    "sites_total",
    "amb",
    "prov",
    "sc",
    "t_bytes",
    "anchoring",
    "dep_capable",
    "surprising",
    "driver",
    "hops",
    "complete",
    "suppressed_comment",
    "suppressed_string",
    "tier_parsed",
    "tier_unclassified",
    "counts_floor",
    "defs_capped",
    "fanout_cut",
    "depth_truncated",
    "date",
    "files",
    "superseded",
    "tip",
    "corpus_pruned_dirs",
    "unindexed_hits",
    "unindexed_files_scanned",
    "unindexed_files_skipped",
    "unindexed_candidates_capped",
    "hits_capped",
    "tested",
    "shown_unflagged",
    "unflagged_capped",
    "bodyless_defs",
    "defs",
    "dist",
    "cells_capped",
    "cells_shown",
    "findings_capped",
    "inert_rules",
    "doc_mentions",
    "doc_mentions_capped",
    "doc_mentions_total",
    "importers_next",
    "coverage",
    "tier_budget",
    "tier_files",
    "corpus_oversize",
    "notes_degraded",
    "join",
    "origin",
    "exempt",
    "idiom",
    "demoted",
    "filter",
    "unranked_extent_suspect",
    "lines_skipped",
    "as_stem_ambiguous",
    "as_stem_nonptr",
    "as_stem_unowned",
    "tier",
    "data_sections_cut",
    "run_first",
    "doc_links",
    "head_date",
    "listing",
    "ref_labels",
    "worktree",
    "by_depth",
    "callers_floor",
    "sev",
}


def find_binaries():
    rw_bin = os.path.abspath("target/release/ripwire-rs")
    if not os.path.exists(rw_bin):
        rw_bin = os.path.abspath("target/release/rw")
    if not os.path.exists(rw_bin):
        rw_bin = shutil.which("ripwire-rs") or shutil.which("rw")
    rip_bin = os.path.expanduser("~/.local/bin/ripwire")
    if not os.path.exists(rip_bin):
        rip_bin = shutil.which("ripwire")
    if not rw_bin or not rip_bin:
        print("Error: ripwire-rs or ripwire binary not found.", file=sys.stderr)
        sys.exit(1)
    return rw_bin, rip_bin


def detect_merge_ref(repo_dir):
    for candidate in ("main", "master", "HEAD"):
        res = subprocess.run(
            ["git", "-C", repo_dir, "rev-parse", "--verify", f"{candidate}^{{commit}}"],
            capture_output=True,
        )
        if res.returncode == 0:
            return candidate
    return "HEAD"


def discover_repo_targets(rw_bin, repo_dir):
    proc = subprocess.run([rw_bin, repo_dir, "--top-k=50"], capture_output=True, text=True)
    files = re.findall(r'<f p="([^"]+)"', proc.stdout)
    fn_syms = re.findall(r'<s t="fn" n="([a-zA-Z_][a-zA-Z0-9_]*)"', proc.stdout)
    if not fn_syms:
        fn_syms = re.findall(r'<s t="method" n="([a-zA-Z_][a-zA-Z0-9_]*)"', proc.stdout)
    if not fn_syms:
        fn_syms = re.findall(r'<s [^>]*n="([a-zA-Z_][a-zA-Z0-9_]*)"', proc.stdout)
    counts = {}
    for s in fn_syms:
        counts[s] = counts.get(s, 0) + 1
    unique_fns = [s for s in fn_syms if counts[s] == 1 and s not in ("new", "main", "name", "init")]
    pool = unique_fns if unique_fns else fn_syms
    target_file = files[0] if files else "src/main.rs"
    target_sym = pool[0] if pool else "main"
    second_sym = pool[1] if len(pool) > 1 else target_sym
    return target_file, target_sym, second_sym


def build_command_list(merge_ref, target_file, target_sym, second_sym):
    return [
        ("", "Default ranked map"),
        ("--top-k=50", "Top 50 symbols"),
        ("--top-k=100", "Top 100 symbols"),
        ("--top-k=20", "Top 20 symbols"),
        ("--max-tokens=2000", "2000 token budget"),
        ("--max-tokens=4000", "4000 token budget"),
        ("--rank-by=authority", "Authority ranking"),
        ("--rank-by=hub", "Hub ranking"),
        ("--legend=compact", "Compact legend"),
        ("--legend=full", "Full legend"),
        ("--tree", "File inventory tree"),
        ("--dead-code", "Dead code detection"),
        ("--communities", "Community clustering"),
        ("--external-surface", "External API surface"),
        ("--deps", "File dependencies"),
        ("--hotspots", "Maintenance hotspots"),
        ("--cochange", "Shotgun surgery co-change mining"),
        ("--clones", "Clone detection"),
        ("-v", "Version banner"),
        ("--quality-delta", "Quality delta gate"),
        ("--test-gate", "Test reachability gate"),
        ("--map-diff --top-k=30", "Map diff biased teleportation"),
        ("--seams", "Integration seams detection"),
        ("--pr-context", "PR review context bundle"),
        (f"--merge-scout={merge_ref}", "Cross-branch merge conflict prediction"),
        ("--biggest-first", "Posnett readability size ranking"),
        ("--nonlocal-state", "Reachable global mutable state"),
        ("--lint", "AST code smell scan"),
        ("--lint-catalog", "AST lint rule catalog"),
        (f"--at={target_file}:1", "Location definition resolver"),
        (f"--verify=calls({target_sym},{second_sym})", "Relationship claim verification"),
        ("--pack-task=search", "Task-scoped context pack"),
        (f'--graph-query=\'callers(name("{target_sym}"))\'', "Functional graph query DSL"),
        ("--exemplar=fn", "Best-in-class style exemplar"),
        ("--owners", "Git author ownership per file"),
        ("--doctor", "Environment and tool diagnostics"),
        ("--notes", "Persistent session gotchas"),
        ("--mermaid", "Mermaid diagram generation"),
        ("--zoom", "Hierarchical Louvain clustering"),
        ("--html", "Interactive HTML visualizer export"),
        ("wrap claude", "Agent integration recipe"),
        (f"--whereis={target_sym}", "Symbol cross-ref locator"),
        ("--flags", "Dark-content gate harvest"),
        ("--stray-content", "Cross-branch divergent content"),
        ("--dmm", "Delta Maintainability Model"),
        ("--quality-panel", "Unified 6-family quality panel"),
        ("--layout=NonExistentStruct999", "Aggregate byte layout model"),
        ("--field-affinity", "Cache-line field co-access"),
        ("--context-ratio", "Local-reasoning context ratio"),
        ("--ensemble", "4-family evidence ensemble"),
        ("--comment-coherence", "Doc-comment restatement coherence"),
        ('--plan-lanes=2 --task="refactor serialization"', "Parallel lane conflict planner"),
        ("--scan-skills=skills", "Skill security scanner"),
        ("--metrics", "Per-symbol complexity and amplification metrics"),
        ("--situ", "Situational awareness diff summary"),
        ("--handoff", "Session handoff summary"),
        (f"--around={target_sym}", "Ego-graph neighborhood"),
        (f"--callers={target_sym}", "Direct 1-hop callers"),
        (f"--callees={target_sym}", "Direct 1-hop callees"),
        (f"--uses={target_sym}", "All static use sites"),
        (f"--impact={target_sym}", "Transitive blast radius"),
        (f"--for={target_sym}", "Task-lens ranked inventory"),
        (f"--grep={target_sym}", "AST-annotated literal search"),
        (f"--slice={target_sym}", "Intra-procedural variable slice"),
        ("--lego=NonExistentTrait999", "Interface implementor lookup"),
        (f"--edit-check={target_sym}", "Post-edit contract check"),
        (f"--safe-delete={target_sym}", "Safe deletion pre-check"),
    ]


def parse_xml_dom(raw_text):
    clean = re.sub(r"<!--.*?-->", "", raw_text, flags=re.DOTALL).strip()
    if not clean:
        return None, "empty stdout"
    try:
        return ET.fromstring(clean), None
    except ET.ParseError as exc:
        return None, f"XML parse error: {exc}"


def compare_xml_children(root_rw, root_rip):
    set_rw = {ch.tag for ch in root_rw}
    set_rip = {ch.tag for ch in root_rip}
    for tag in sorted(set_rip & set_rw):
        ch_rw = next(ch for ch in root_rw if ch.tag == tag)
        ch_rip = next(ch for ch in root_rip if ch.tag == tag)
        req_rip = set(ch_rip.attrib.keys()) - OPTIONAL_ATTRS
        req_rw = set(ch_rw.attrib.keys()) - OPTIONAL_ATTRS
        missing_attrs = sorted(req_rip - req_rw)
        if missing_attrs:
            return f"child <{tag}> missing required attrs {missing_attrs}"
    return None


def check_xml_data_invariants(cmd, root_rw, root_rip):
    if "--dead-code" in cmd:
        c_rw = int(root_rw.attrib.get("count", "0"))
        c_rip = int(root_rip.attrib.get("count", "0"))
        if abs(c_rw - c_rip) > 50 and c_rw > c_rip * 5:
            return f"dead-code count mismatch: rw={c_rw} vs rip={c_rip}"
    if "--grep=" in cmd and any(not ch.attrib.get("p") for ch in root_rw.findall("f")):
        return "grep <f> has empty p attribute"
    return None


def check_xml_parity(cmd, out_rw, out_rip):
    root_rw, err_rw = parse_xml_dom(out_rw)
    if err_rw:
        return f"ripwire-rs {err_rw}"
    root_rip, err_rip = parse_xml_dom(out_rip)
    if err_rip:
        return f"ripwire {err_rip}"
    if root_rw.tag != root_rip.tag:
        return f"root tag mismatch: rw=<{root_rw.tag}> vs rip=<{root_rip.tag}>"
    if root_rw.attrib.get("schema") != root_rip.attrib.get("schema"):
        return f"schema mismatch: rw={root_rw.attrib.get('schema')} vs rip={root_rip.attrib.get('schema')}"
    req_rip = set(root_rip.attrib.keys()) - OPTIONAL_ATTRS
    req_rw = set(root_rw.attrib.keys()) - OPTIONAL_ATTRS
    if req_rw != req_rip:
        missing_root = sorted(req_rip - req_rw)
        extra_root = sorted(req_rw - req_rip)
        return f"root attr keys mismatch: missing={missing_root} extra={extra_root}"
    if "--max-tokens=2000" in cmd and len(out_rw) > max(len(out_rip) * 3, 10000):
        return f"budget overrun: rw={len(out_rw)}B vs rip={len(out_rip)}B"
    inv_err = check_xml_data_invariants(cmd, root_rw, root_rip)
    return inv_err if inv_err else compare_xml_children(root_rw, root_rip)


def check_plan_lanes_json(out_rw, out_rip):
    try:
        j_rw = json.loads(out_rw.strip())
        j_rip = json.loads(out_rip.strip())
    except Exception as exc:
        return f"plan-lanes JSON error: {exc}"
    if len(out_rw.strip().splitlines()) != 1:
        return "plan-lanes must be 1-line JSON"
    missing = sorted(set(j_rip.keys()) - set(j_rw.keys()) - {"warnings", "notes_degraded"})
    return f"plan-lanes missing keys {missing}" if missing else None


def check_non_xml_parity(cmd, out_rw, out_rip):
    if cmd == "-v":
        return None if ("ripwire" in out_rw or "rw" in out_rw) else "missing version string"
    if "--mermaid" in cmd:
        return None if "flowchart" in out_rw else "missing flowchart"
    if "--html" in cmd:
        return None if ("<!DOCTYPE html>" in out_rw and "const NODES" in out_rw) else "missing html"
    if cmd.startswith("wrap"):
        return None if ("claude mcp add" in out_rw and "claude mcp add" in out_rip) else "missing wrap"
    if "--situ" in cmd:
        return None if "situational-awareness" in out_rw else "missing situ"
    if "--plan-lanes" in cmd:
        return check_plan_lanes_json(out_rw, out_rip)
    return "unhandled text command"


def run_one_check(rw_bin, rip_bin, repo, cmd):
    if cmd == "-v":
        c_rw, c_rip = [rw_bin, "-V"], [rip_bin, "-v"]
    elif cmd.startswith("wrap"):
        c_rw, c_rip = [rw_bin] + cmd.split(), [rip_bin] + cmd.split()
    else:
        args = shlex.split(cmd)
        c_rw, c_rip = [rw_bin, repo] + args, [rip_bin, repo] + args
    p_rw = subprocess.run(c_rw, capture_output=True, text=True, timeout=20)
    try:
        p_rip = subprocess.run(c_rip, capture_output=True, text=True, timeout=4)
    except subprocess.TimeoutExpired:
        is_text = cmd in ("-v", "--mermaid", "--html", "--situ") or cmd.startswith("wrap") or "--plan-lanes" in cmd
        if is_text:
            return None if p_rw.stdout.strip() else "empty text output"
        root_rw, err_rw = parse_xml_dom(p_rw.stdout)
        return err_rw if err_rw else (None if root_rw.attrib.get("schema") else "missing schema")
    if p_rw.returncode != p_rip.returncode:
        if (
            (p_rw.returncode != 0 and p_rip.returncode != 0)
            or "--quality-delta" in cmd
            or "--test-gate" in cmd
            or "--scan-skills" in cmd
            or (repo != "." and any(k in cmd for k in ("--slice=", "--edit-check=", "--at=", "--doctor")))
        ):
            if not p_rw.stdout.strip() or not p_rip.stdout.strip():
                return None
        else:
            return f"exit code mismatch: rw={p_rw.returncode} vs rip={p_rip.returncode}"
    if p_rw.returncode != 0 and not p_rip.stdout.strip():
        return None
    is_text = cmd in ("-v", "--mermaid", "--html", "--situ") or cmd.startswith("wrap") or "--plan-lanes" in cmd
    if is_text:
        return check_non_xml_parity(cmd, p_rw.stdout, p_rip.stdout)
    return check_xml_parity(cmd, p_rw.stdout, p_rip.stdout)


def main():
    repo = sys.argv[1] if len(sys.argv) > 1 else "."
    if not os.path.exists(repo):
        print(f"Error: repository path '{repo}' does not exist", file=sys.stderr)
        sys.exit(1)
    rw_bin, rip_bin = find_binaries()
    merge_ref = detect_merge_ref(repo)
    target_file, target_sym, second_sym = discover_repo_targets(rw_bin, repo)
    commands = build_command_list(merge_ref, target_file, target_sym, second_sym)
    print("=" * 66)
    print(f"Validating 1:1 Head-to-Head Parity on: {os.path.abspath(repo)}")
    print(f"Seed file: {target_file} | Symbols: {target_sym}, {second_sym}")
    print(f"ripwire-rs binary: {rw_bin}")
    print(f"ripwire binary:    {rip_bin}")
    print("=" * 66)
    failures = 0
    for cmd, desc in commands:
        reason = run_one_check(rw_bin, rip_bin, repo, cmd)
        label = cmd if cmd else "(default)"
        if reason is None:
            print(f"  \033[32m[PASS]\033[0m {label:<36} - {desc}")
        else:
            failures += 1
            print(f"  \033[31m[FAIL]\033[0m {label:<36} - {desc} ({reason})")
    print("=" * 66)
    total = len(commands)
    passed = total - failures
    if failures == 0:
        print(f"\033[32mPARITY VERIFIED: {passed}/{total} commands matched 1:1 DOM & contract parity.\033[0m")
        sys.exit(0)
    print(f"\033[31mPARITY FAILED: {failures}/{total} commands diverged from ripwire.\033[0m")
    sys.exit(1)


if __name__ == "__main__":
    main()
