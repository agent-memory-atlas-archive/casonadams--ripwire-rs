#!/usr/bin/env python3
import importlib.util
import os
import re
import subprocess
import sys

_SPEC = importlib.util.spec_from_file_location(
    "bench_perf", os.path.join(os.path.dirname(__file__), "benchmark-performance.py")
)
_MOD = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MOD)
locate_bench_binaries = _MOD.locate_bench_binaries
measure_process = _MOD.measure_process

def find_git_repos(path):
    if os.path.isdir(os.path.join(path, ".git")):
        return [path]
    repos = []
    for entry in sorted(os.listdir(path)):
        full = os.path.join(path, entry)
        if os.path.isdir(os.path.join(full, ".git")):
            repos.append(full)
    return repos


def pick_repo_symbol(rs_bin, repo_path):
    proc = subprocess.run([rs_bin, repo_path, "--top-k=5"], capture_output=True, text=True)
    syms = re.findall(r'<s [^>]*n="([a-zA-Z0-9_]+)"', proc.stdout)
    for sym in syms:
        if len(sym) > 2:
            return sym
    return "main"


def run_repo_suite(rs_bin, cpp_bin, repo_path):
    repo_name = os.path.basename(repo_path)
    sym = pick_repo_symbol(rs_bin, repo_path)
    print(f"\n--- Repository: {repo_name} (Symbol: {sym}) ---")
    print("Operation        | RS Wall | C++ Wall|   RS CPU|  C++ CPU|  RS RSS | C++ RSS | Verdict")
    print("-" * 88)
    subprocess.run([rs_bin, repo_path], capture_output=True)
    subprocess.run([cpp_bin, repo_path], capture_output=True)
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
    fmt = "{:<16} | {:>6.3f}s | {:>6.3f}s | {:>6.3f}s | {:>6.3f}s | {:>5.1f}MB | {:>5.1f}MB | {}"
    stats = [0, 0, 0, 0]
    for name, args in suite:
        stats[0] += 1
        _, rs_w, rs_c, rs_m = measure_process([rs_bin, repo_path] + args)
        _, cpp_w, cpp_c, cpp_m = measure_process([cpp_bin, repo_path] + args)
        ok_w = rs_w <= max(cpp_w * 1.15, cpp_w + 0.02)
        ok_c = rs_c <= max(cpp_c * 1.15, cpp_c + 0.02)
        ok_m = rs_m <= max(cpp_m * 1.20, cpp_m + 2.0)
        stats[1] += int(ok_w)
        stats[2] += int(ok_c)
        stats[3] += int(ok_m)
        verdict = "\033[32mPASS\033[0m" if (ok_w and ok_c and ok_m) else "\033[33mWARN\033[0m"
        print(fmt.format(name, rs_w, cpp_w, rs_c, cpp_c, rs_m, cpp_m, verdict))
    return stats


def main():
    target_dir = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else "..")
    rs_bin, cpp_bin = locate_bench_binaries()
    repos = find_git_repos(target_dir)
    if not repos:
        print(f"Error: no git repositories found in {target_dir}", file=sys.stderr)
        sys.exit(1)
    totals = [0, 0, 0, 0]
    for repo_path in repos:
        res = run_repo_suite(rs_bin, cpp_bin, repo_path)
        for idx in range(4):
            totals[idx] += res[idx]
    runs = max(totals[0], 1)
    w_pct, c_pct, m_pct = (totals[1] * 100 / runs), (totals[2] * 100 / runs), (totals[3] * 100 / runs)
    print("\n" + "=" * 88)
    print(f"Summary: {totals[0]} runs | Wall: {w_pct:.1f}% | CPU: {c_pct:.1f}% | RSS: {m_pct:.1f}%")
    if min(w_pct, c_pct, m_pct) < 85.0:
        sys.exit(1)


if __name__ == "__main__":
    main()
