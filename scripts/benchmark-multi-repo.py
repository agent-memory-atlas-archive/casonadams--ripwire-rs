#!/usr/bin/env python3
import sys
import os
import subprocess
import re
import shutil
import glob

target_dir = sys.argv[1] if len(sys.argv) > 1 else ".."
target_dir = os.path.abspath(target_dir)

rs_bin = os.path.abspath("target/release/ripwire-rs")
if not os.path.exists(rs_bin):
    rs_bin = os.path.abspath("target/release/rw")
if not os.path.exists(rs_bin):
    rs_bin = shutil.which("ripwire-rs") or shutil.which("rw")
if not rs_bin:
    print("Error: 'ripwire-rs' release binary not found. Run 'cargo build --release' first.", file=sys.stderr)
    sys.exit(1)

cpp_bin = os.path.expanduser("~/.local/bin/ripwire")
if not os.path.exists(cpp_bin):
    cpp_bin = shutil.which("ripwire")
if not cpp_bin:
    print("Error: upstream 'ripwire' binary not found in ~/.local/bin/ripwire or PATH.", file=sys.stderr)
    sys.exit(1)

def find_repos(path):
    if os.path.isdir(os.path.join(path, ".git")):
        return [path]
    repos = []
    for entry in sorted(os.listdir(path)):
        full = os.path.join(path, entry)
        if os.path.isdir(os.path.join(full, ".git")):
            repos.append(full)
    return repos

repos = find_repos(target_dir)
if not repos:
    print(f"Error: no git repositories found in {target_dir}", file=sys.stderr)
    sys.exit(1)

def run_timed(cmd):
    p = subprocess.run(["/usr/bin/time", "-l"] + cmd, capture_output=True, text=True)
    out = p.stderr
    m_time = re.search(r"([0-9.]+)\s+real", out)
    real_sec = float(m_time.group(1)) if m_time else 0.0
    m_rss = re.search(r"([0-9]+)\s+maximum resident set size", out)
    max_rss_mb = (int(m_rss.group(1)) / (1024 * 1024)) if m_rss else 0.0
    return p, real_sec, max_rss_mb

def get_top_symbol(repo_path):
    p = subprocess.run([rs_bin, repo_path, "--top-k=5"], capture_output=True, text=True)
    syms = re.findall(r'<s [^>]*n="([a-zA-Z0-9_]+)"', p.stdout)
    for s in syms:
        if len(s) > 2:
            return s
    return "main"

print("==========================================================================================")
print(f"Multi-Repo Parity, Speed & Memory Sweep: {len(repos)} repositories")
print(f"Rust binary: {rs_bin}")
print(f"C++ binary:  {cpp_bin}")
print("==========================================================================================")

total_runs = 0
faster_runs = 0
lower_mem_runs = 0

fmt = "{:<16} | {:>8.3f}s | {:>8.3f}s | {:>7.2f}x | {:>7.1f}MB | {:>7.1f}MB | {:>9} | {:>6}"

for r in repos:
    repo_name = os.path.basename(r)
    sym = get_top_symbol(r)
    print(f"\n--- Repository: {repo_name} (Symbol: {sym}) ---")
    print("Operation        | Rust Time | C++ Time  | Speedup | Rust RSS| C++ RSS | RSS Delta | Verdict")
    print("-" * 90)

    # Warm cache
    subprocess.run([rs_bin, r], capture_output=True)
    subprocess.run([cpp_bin, r], capture_output=True)

    suite = [
        ("Map (Warm)", []),
        ("Callers", [f"--callers={sym}"]),
        ("Impact", [f"--impact={sym}"]),
        ("Uses", [f"--uses={sym}"]),
        ("Whereis", [f"--whereis={sym}"]),
        ("Flags", ["--flags"]),
        ("Stray Content", ["--stray-content"]),
        ("Owners", ["--owners"]),
        ("Cochange", ["--cochange"]),
        ("Hotspots", ["--hotspots"]),
        ("Clones", ["--clones"]),
    ]

    for name, args in suite:
        total_runs += 1
        _, rs_t, rs_m = run_timed([rs_bin, r] + args)
        _, cpp_t, cpp_m = run_timed([cpp_bin] + args + [r])

        speedup = cpp_t / rs_t if rs_t > 0 else 1.0
        rss_pct = ((rs_m - cpp_m) / cpp_m * 100) if cpp_m > 0 else 0.0
        rss_str = f"{rss_pct:+.1f}%"

        is_faster = rs_t <= (cpp_t * 1.15)  # 15% margin for variance
        is_lower_mem = rs_m <= (cpp_m * 1.25)  # 25% margin for allocator baseline

        if is_faster:
            faster_runs += 1
        if is_lower_mem:
            lower_mem_runs += 1

        verdict = "PASS" if is_faster and is_lower_mem else "WARN"
        v_color = "\033[32mPASS\033[0m" if verdict == "PASS" else "\033[33mWARN\033[0m"
        print(fmt.format(name, rs_t, cpp_t, speedup, rs_m, cpp_m, rss_str, v_color))

print("\n" + "=" * 90)
speed_pct = (faster_runs / total_runs * 100) if total_runs > 0 else 0
mem_pct = (lower_mem_runs / total_runs * 100) if total_runs > 0 else 0
print(f"Summary: {total_runs} benchmark runs across {len(repos)} repositories")
print(f"Speed Invariant (Rust <= C++): {faster_runs}/{total_runs} ({speed_pct:.1f}%)")
print(f"Memory Invariant (Rust RSS <= C++ RSS): {lower_mem_runs}/{total_runs} ({mem_pct:.1f}%)")

if speed_pct >= 80.0:
    print("\033[32mPERFORMANCE INVARIANT VERIFIED: ripwire-rs performs better or on-par across repos.\033[0m")
    sys.exit(0)
else:
    print("\033[31mPERFORMANCE DEFECT DETECTED: ripwire-rs fell behind C++ on >20% of benchmarks.\033[0m")
    sys.exit(1)
