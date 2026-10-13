#!/usr/bin/env python3
import sys
import os
import subprocess
import re
import shutil
import time

repo = sys.argv[1] if len(sys.argv) > 1 else "."

if not os.path.exists(repo):
    print(f"Error: repository path '{repo}' does not exist", file=sys.stderr)
    sys.exit(1)

rs_bin = os.path.abspath("target/release/ripwire-rs")
if not os.path.exists(rs_bin):
    rs_bin = os.path.abspath("target/release/rw")
if not os.path.exists(rs_bin):
    rs_bin = shutil.which("ripwire-rs") or shutil.which("rw")
    if not rs_bin:
        print("Error: 'ripwire-rs' binary not found. Run 'make build-release' first.", file=sys.stderr)
        sys.exit(1)

cpp_bin = shutil.which("ripwire")
if not cpp_bin:
    candidate = os.path.expanduser("~/.local/bin/ripwire")
    if os.path.exists(candidate):
        cpp_bin = candidate
    else:
        print("Error: 'ripwire' binary not found in PATH or ~/.local/bin/ripwire.", file=sys.stderr)
        sys.exit(1)

def run_timed(cmd):
    p = subprocess.run(["/usr/bin/time", "-l"] + cmd, capture_output=True, text=True)
    out = p.stderr
    m_time = re.search(r"([0-9.]+)\s+real", out)
    real_sec = float(m_time.group(1)) if m_time else 0.0
    m_rss = re.search(r"([0-9]+)\s+maximum resident set size", out)
    max_rss_mb = (int(m_rss.group(1)) / (1024 * 1024)) if m_rss else 0.0
    return p, real_sec, max_rss_mb

# Discover top symbols
p_sym = subprocess.run([rs_bin, repo], capture_output=True, text=True)
syms = re.findall(r"<s t=\"(?:fn|method)\" n=\"([a-zA-Z0-9_]+)\"", p_sym.stdout)
if not syms:
    syms = re.findall(r"<s [^>]*n=\"([a-zA-Z0-9_]+)\"", p_sym.stdout)
seen = set()
uniq = []
for s in syms:
    if s not in seen and len(s) > 2:
        seen.add(s)
        uniq.append(s)
if len(uniq) == 0:
    uniq = ["main", "init"]
elif len(uniq) == 1:
    uniq.append(uniq[0])
sym1, sym2 = uniq[0], uniq[1]

# Warm up auto-caches
subprocess.run([rs_bin, repo], capture_output=True)
subprocess.run([cpp_bin, repo], capture_output=True)

commands = [
    ("Map (Warm)", []),
    ("Map (Cold)", ["--no-cache"]),
    ("Callers", [f"--callers={sym1}"]),
    ("Callees", [f"--callees={sym1}"]),
    ("Impact", [f"--impact={sym1}"]),
    ("Uses", [f"--uses={sym1}"]),
    ("For Task", [f"--for=explore and inspect {sym1}"]),
    ("Grep", [f"--grep={sym1}"]),
    ("Connect", [f"--connect={sym1},{sym2}"]),
    ("Path", [f"--path={sym1},{sym2}"]),
    ("Whereis", [f"--whereis={sym1}"]),
    ("Doc Drift", ["--doc-drift"]),
    ("Flags", ["--flags"]),
    ("Stray Content", ["--stray-content"]),
    ("Mentions", [f"--mentions={sym1}"]),
    ("AST Clones", ["--clones"]),
    ("Cochange", ["--cochange"]),
    ("Hotspots", ["--hotspots"]),
]

fmt = "{:<16} | {:>9.3f}s | {:>9.3f}s | {:>7.2f}x | {:>8.1f} MB | {:>8.1f} MB | {:>10}"
print(f"========================================================================================")
print(f"Performance & Resource Benchmark: {os.path.abspath(repo)}")
print(f"Symbols: {sym1}, {sym2}")
print(f"Rust: {rs_bin} | C++: {cpp_bin}")
print(f"========================================================================================")
print("Operation        |  Rust Time |   C++ Time | Speedup |   Rust RSS |    C++ RSS |  RSS Delta")
print("-" * 88)

for name, args in commands:
    p_rs, rs_t, rs_m = run_timed([rs_bin, repo] + args)
    p_cpp, cpp_t, cpp_m = run_timed([cpp_bin, repo] + args)
    speedup = cpp_t / rs_t if rs_t > 0 else 1.0
    mem_delta = f"{((rs_m - cpp_m) / cpp_m * 100):+.1f}%"
    print(fmt.format(name, rs_t, cpp_t, speedup, rs_m, cpp_m, mem_delta))
print("=" * 88)
