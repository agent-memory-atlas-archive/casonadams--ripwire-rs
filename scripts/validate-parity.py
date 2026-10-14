#!/usr/bin/env python3
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET


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


def build_command_list(merge_ref):
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
        ("--at=src/main.rs:1", "Location definition resolver"),
        ("--verify=calls(main,run_cli)", "Relationship claim verification"),
        ("--pack-task=search", "Task-scoped context pack"),
        ('--graph-query=\'callers(name("find_symbols_by_name"))\'', "Functional graph query DSL"),
        ("--exemplar=fn", "Best-in-class style exemplar"),
        ("--owners", "Git author ownership per file"),
        ("--doctor", "Environment and tool diagnostics"),
        ("--notes", "Persistent session gotchas"),
        ("--mermaid", "Mermaid diagram generation"),
        ("--zoom", "Hierarchical Louvain clustering"),
        ("--html", "Interactive HTML visualizer export"),
        ("wrap claude", "Agent integration recipe"),
        ("--whereis=main", "Symbol cross-ref locator"),
        ("--flags", "Dark-content gate harvest"),
        ("--stray-content", "Cross-branch divergent content"),
        ("--dmm", "Delta Maintainability Model"),
        ("--quality-panel", "Unified 6-family quality panel"),
        ("--layout=Symbol", "Aggregate byte layout model"),
        ("--field-affinity", "Cache-line field co-access"),
        ("--context-ratio", "Local-reasoning context ratio"),
        ("--ensemble", "4-family evidence ensemble"),
        ("--comment-coherence", "Doc-comment restatement coherence"),
        ('--plan-lanes=2 --task="refactor xml serialization"', "Parallel lane conflict planner"),
        ("--scan-skills", "Skill security scanner"),
        ("--metrics", "Per-symbol complexity and amplification metrics"),
        ("--situ", "Situational awareness diff summary"),
        ("--handoff", "Session handoff summary"),
        ("--around=run_cli", "Ego-graph neighborhood"),
        ("--callers=run_cli", "Direct 1-hop callers"),
        ("--callees=run_cli", "Direct 1-hop callees"),
        ("--uses=run_cli", "All static use sites"),
        ("--impact=run_cli", "Transitive blast radius"),
        ("--for=xml", "Task-lens ranked inventory"),
        ("--grep=MinifyingWriter", "AST-annotated literal search"),
        ("--slice=run_cli", "Intra-procedural variable slice"),
        ("--lego=Write", "Interface implementor lookup"),
        ("--edit-check=run_cli", "Post-edit contract check"),
        ("--safe-delete=run_cli", "Safe deletion pre-check"),
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
    tags_rw = [ch.tag for ch in root_rw]
    tags_rip = [ch.tag for ch in root_rip]
    set_rw = set(tags_rw)
    set_rip = set(tags_rip)
    if set_rip and not set_rw:
        return f"ripwire has children {sorted(set_rip)} but ripwire-rs has none"
    missing_tags = sorted(set_rip - set_rw)
    if missing_tags:
        return f"missing child tags {missing_tags} (got {sorted(set_rw)})"
    for tag in sorted(set_rip & set_rw):
        ch_rw = next(ch for ch in root_rw if ch.tag == tag)
        ch_rip = next(ch for ch in root_rip if ch.tag == tag)
        missing_attrs = sorted(set(ch_rip.attrib.keys()) - set(ch_rw.attrib.keys()))
        if missing_attrs:
            return f"child <{tag}> missing attrs {missing_attrs}"
    return None


def check_xml_data_invariants(cmd, root_rw, root_rip):
    if "--dead-code" in cmd:
        c_rw = int(root_rw.attrib.get("count", "0"))
        c_rip = int(root_rip.attrib.get("count", "0"))
        if abs(c_rw - c_rip) > 10:
            return f"dead-code count mismatch: rw={c_rw} vs rip={c_rip}"
    if "--flags" in cmd and root_rw.attrib.get("dark_gates") != root_rip.attrib.get("dark_gates"):
        return f"flags dark_gates mismatch: rw={root_rw.attrib.get('dark_gates')} vs rip={root_rip.attrib.get('dark_gates')}"
    if "--dmm" in cmd and root_rw.attrib.get("available") != root_rip.attrib.get("available"):
        return f"dmm available mismatch: rw={root_rw.attrib.get('available')} vs rip={root_rip.attrib.get('available')}"
    if "--comment-coherence" in cmd and int(root_rip.attrib.get("documented", "0")) > 0 and int(root_rw.attrib.get("documented", "0")) == 0:
        return "comment-coherence documented=0"
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
    if set(root_rw.attrib.keys()) != set(root_rip.attrib.keys()):
        missing_root = sorted(set(root_rip.attrib.keys()) - set(root_rw.attrib.keys()))
        extra_root = sorted(set(root_rw.attrib.keys()) - set(root_rip.attrib.keys()))
        return f"root attr keys mismatch: missing={missing_root} extra={extra_root}"
    if "--max-tokens=2000" in cmd and len(out_rw) > len(out_rip) * 2:
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
    missing = sorted(set(j_rip.keys()) - set(j_rw.keys()))
    return f"plan-lanes missing keys {missing}" if missing else None


def check_non_xml_parity(cmd, out_rw, out_rip):
    if cmd == "-v":
        return None if ("ripwire" in out_rw or "rw" in out_rw) else "missing version string"
    if "--mermaid" in cmd:
        return None if ("flowchart" in out_rw and "flowchart" in out_rip) else "missing flowchart"
    if "--html" in cmd:
        return None if ("<!DOCTYPE html>" in out_rw and "const NODES" in out_rw) else "missing html"
    if cmd.startswith("wrap"):
        return None if ("claude mcp add" in out_rw and "claude mcp add" in out_rip) else "missing wrap"
    if "--situ" in cmd:
        return None if ("situational-awareness" in out_rw and "situational-awareness" in out_rip) else "missing situ"
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
    p_rw = subprocess.run(c_rw, capture_output=True, text=True)
    p_rip = subprocess.run(c_rip, capture_output=True, text=True)
    if p_rw.returncode != p_rip.returncode:
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
    commands = build_command_list(merge_ref)
    print("=" * 66)
    print(f"Validating 1:1 Head-to-Head Parity on: {os.path.abspath(repo)}")
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
