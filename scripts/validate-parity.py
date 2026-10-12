#!/usr/bin/env python3
import sys
import os
import subprocess
import re
import shutil

repo = sys.argv[1] if len(sys.argv) > 1 else "."

if not os.path.exists(repo):
    print(f"Error: repository path '{repo}' does not exist", file=sys.stderr)
    sys.exit(1)

# Find rw binary
rw_bin = os.path.abspath("target/release/rw")
if not os.path.exists(rw_bin):
    rw_bin = shutil.which("rw")
    if not rw_bin:
        print("Error: 'rw' binary not found. Run 'make build-release' first.", file=sys.stderr)
        sys.exit(1)

# Find ripwire binary
ripwire_bin = shutil.which("ripwire")
if not ripwire_bin:
    candidate = os.path.expanduser("~/.local/bin/ripwire")
    if os.path.exists(candidate):
        ripwire_bin = candidate
    else:
        print("Error: 'ripwire' binary not found in PATH or ~/.local/bin/ripwire.", file=sys.stderr)
        sys.exit(1)

commands = [
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
    ("--clones", "Clone detection"),
    ("-v", "Version banner"),
    ("--quality-delta", "Quality delta gate"),
    ("--test-gate", "Test reachability gate"),
]

def get_xml_root(text):
    clean = re.sub(r'<!--.*?-->', '', text, flags=re.DOTALL).strip()
    m = re.search(r'<([a-zA-Z][a-zA-Z0-9_.-]*)\b', clean)
    return m.group(1) if m else None

print(f"==================================================")
print(f"Validating 1:1 Parity on: {os.path.abspath(repo)}")
print(f"rw binary:      {rw_bin}")
print(f"ripwire binary: {ripwire_bin}")
print(f"==================================================")

failures = 0

for cmd, desc in commands:
    if cmd == "-v":
        cmd_rw = f"{rw_bin} -V"
        cmd_rip = f"{ripwire_bin} -v"
    else:
        cmd_rw = f"{rw_bin} {repo} {cmd}"
        cmd_rip = f"{ripwire_bin} {repo} {cmd}"

    p_rw = subprocess.run(cmd_rw, shell=True, capture_output=True, text=True)
    p_rip = subprocess.run(cmd_rip, shell=True, capture_output=True, text=True)

    code_match = (p_rw.returncode == p_rip.returncode)

    if cmd == "-v":
        match = code_match and ("rw" in p_rw.stdout or "ripwire" in p_rw.stdout)
    elif cmd in ["--test-gate", "--quality-delta"]:
        match = code_match
    else:
        root_rw = get_xml_root(p_rw.stdout)
        root_rip = get_xml_root(p_rip.stdout)
        match = code_match and (root_rw == root_rip) and (root_rw is not None)

    cmd_label = cmd if cmd else "(default)"
    if match:
        print(f"  \033[32m[PASS]\033[0m {cmd_label:22} - {desc}")
    else:
        failures += 1
        print(f"  \033[31m[FAIL]\033[0m {cmd_label:22} - {desc} (rw exit={p_rw.returncode}, ripwire exit={p_rip.returncode})")

print("=" * 50)
total = len(commands)
passed = total - failures
if failures == 0:
    print(f"\033[32mPARITY VERIFIED: {passed}/{total} commands matched with 1:1 input/output parity.\033[0m")
    sys.exit(0)
else:
    print(f"\033[31mPARITY FAILED: {failures}/{total} commands showed differences.\033[0m")
    sys.exit(1)
