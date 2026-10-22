#!/usr/bin/env python3
"""Held-out retrieval evaluation for `--for` conceptual queries.

Reads scripts/retrieval-eval.json, runs each question through one or more
ripwire binaries with `--for=QUERY --signatures-only --token-budget=N`, and
reports where the authoritative targets land in the returned `<d>` rows.

Metrics per split:
  top1   share of questions whose first returned row is authoritative
  mrr    mean reciprocal rank of the first authoritative row (0 if absent)
  hit@k  share with an authoritative row within the first k rows
  pres   share with any authoritative row in the returned set (presence)
  rec    mean fraction of authoritative targets present in the returned set
  bytes  median stdout bytes per query
  ms     median warm wall time per query in milliseconds

Usage:
  scripts/retrieval-eval.py [--bin ours=PATH] [--bin cpp=PATH]
                            [--budget 2000] [--runs 3] [--json OUT]
"""

import argparse
import html
import json
import os
import re
import statistics
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
FIXTURE = os.path.join(HERE, "retrieval-eval.json")
FIXTURE_HELD2 = os.path.join(HERE, "retrieval-eval-held2.json")
FIXTURE_HELD3 = os.path.join(HERE, "retrieval-eval-held3.json")
FIXTURE_REGRESSION = os.path.join(HERE, "retrieval-eval-regression.json")
ROW = re.compile(r"<d\s([^>]*)>")
ATTR = re.compile(r'(\w+)="([^"]*)"')
AUTH = "authoritative"
SPLITS = ("tune", "held", "held2", "held3", "regression")


def parse_rows(text):
    rows = []
    for m in ROW.finditer(text):
        attrs = {k: html.unescape(v) for k, v in ATTR.findall(m.group(1))}
        rows.append((attrs.get("n", ""), attrs.get("p", "")))
    return rows


def run_query(binary, repo, query, budget):
    argv = [
        binary,
        repo,
        f"--for={query}",
        "--signatures-only",
        f"--token-budget={budget}",
    ]
    start = time.perf_counter()
    proc = subprocess.run(argv, capture_output=True, text=True)
    elapsed = (time.perf_counter() - start) * 1000.0
    return proc.stdout, proc.returncode, elapsed


def json_rows(node):
    """Row objects (with `n` and `p`) of the JSON pack, in document order."""
    if isinstance(node, dict):
        if "n" in node and "p" in node:
            yield node
        for value in node.values():
            yield from json_rows(value)
    elif isinstance(node, list):
        for value in node:
            yield from json_rows(value)


def confusion_table(binary, questions, budget):
    """Reported `confidence` against whether the first returned symbol is authoritative."""
    table = {}
    for q in questions:
        argv = [binary, q["repo"], "--json", f"--for={q['query']}", "--signatures-only",
                f"--token-budget={budget}"]
        data = json.loads(subprocess.run(argv, capture_output=True, text=True).stdout or "{}")
        first = next(json_rows(data), None)
        top = (first["n"], first["p"]) if first else None
        auth = {(t["name"], t["path"]) for t in q["targets"] if t["label"] == AUTH}
        key = (data.get("confidence", "none"), top in auth)
        table[key] = table.get(key, 0) + 1
    print("confidence  top1-auth=yes  top1-auth=no")
    for conf in ("high", "medium", "low", "none"):
        print(f"{conf:10}  {table.get((conf, True), 0):>12}  {table.get((conf, False), 0):>11}")
    print()


def score(question, rows, first_auth_pos, auth_targets):
    present = {(n, p) for n, p in rows}
    found = [t for t in auth_targets if (t["name"], t["path"]) in present]
    hit_rank = first_auth_pos
    return {
        "id": question["id"],
        "split": question["split"],
        "rank": hit_rank,
        "present": bool(found),
        "recall": len(found) / len(auth_targets) if auth_targets else 0.0,
        "rows": len(rows),
    }


def evaluate(binary, questions, budget, runs):
    results = []
    for q in questions:
        repo = q["repo"]
        auth = [t for t in q["targets"] if t["label"] == AUTH]
        text, code, _ = run_query(binary, repo, q["query"], budget)
        timings = []
        for _ in range(runs):
            _, _, ms = run_query(binary, repo, q["query"], budget)
            timings.append(ms)
        rows = parse_rows(text)
        auth_keys = {(t["name"], t["path"]) for t in auth}
        pos = next((i + 1 for i, r in enumerate(rows) if r in auth_keys), None)
        res = score(q, rows, pos, auth)
        res["bytes"] = len(text.encode("utf-8"))
        res["ms"] = statistics.median(timings)
        res["exit"] = code
        res["first_row"] = rows[0][0] if rows else None
        results.append(res)
    return results


def summarize(results, questions_by_id):
    out = {}
    for split in SPLITS + ("all",):
        rs = [r for r in results if split == "all" or r["split"] == split]
        if not rs:
            continue
        n = len(rs)
        ranks = [r["rank"] for r in rs]
        out[split] = {
            "n": n,
            "top1": sum(1 for k in ranks if k == 1) / n,
            "hit@3": sum(1 for k in ranks if k is not None and k <= 3) / n,
            "hit@10": sum(1 for k in ranks if k is not None and k <= 10) / n,
            "mrr": sum(1.0 / k for k in ranks if k) / n,
            "pres": sum(1 for r in rs if r["present"]) / n,
            "rec": sum(r["recall"] for r in rs) / n,
            "bytes": statistics.median(r["bytes"] for r in rs),
            "ms": statistics.median(r["ms"] for r in rs),
            "rows": statistics.mean(r["rows"] for r in rs),
            "exit_nonzero": sum(1 for r in rs if r["exit"] != 0),
        }
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[1])
    ap.add_argument("--bin", action="append", default=[], metavar="NAME=PATH")
    ap.add_argument("--budget", type=int, default=2000)
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--split", choices=SPLITS + ("all",), default="all")
    ap.add_argument("--confusion", action="store_true", help="also print confidence vs top-1 table")
    ap.add_argument("--json", metavar="OUT")
    args = ap.parse_args()

    with open(FIXTURE, encoding="utf-8") as fh:
        fixture = json.load(fh)
    extra = []
    for path in (FIXTURE_HELD2, FIXTURE_HELD3, FIXTURE_REGRESSION):
        with open(path, encoding="utf-8") as fh:
            extra += json.load(fh)["questions"]
    repos = fixture["repos"]
    questions = [
        q for q in fixture["questions"] + extra
        if args.split == "all" or q["split"] == args.split
    ]
    for q in questions:
        q["repo"] = repos[q["repo"]]

    bins = {}
    for spec in args.bin or ["ours=" + os.path.join(HERE, "..", "target", "debug", "ripwire-rs")]:
        name, _, path = spec.partition("=")
        bins[name] = path

    report = {}
    for name, path in bins.items():
        results = evaluate(path, questions, args.budget, args.runs)
        report[name] = {"summary": summarize(results, {q["id"]: q for q in questions}),
                        "results": results}

    for split in SPLITS + ("all",):
        print(f"== {split} (budget={args.budget}) ==")
        print(f"{'tool':8} {'n':>3} {'top1':>6} {'hit@3':>6} {'hit@10':>7} {'mrr':>6} "
              f"{'pres':>6} {'rec':>6} {'rows':>5} {'bytes':>8} {'ms':>7} {'exit!=0':>7}")
        for name in bins:
            s = report[name]["summary"].get(split)
            if not s:
                continue
            print(f"{name:8} {s['n']:>3} {s['top1']:>6.2f} {s['hit@3']:>6.2f} {s['hit@10']:>7.2f} "
                  f"{s['mrr']:>6.3f} {s['pres']:>6.2f} {s['rec']:>6.2f} {s['rows']:>5.1f} "
                  f"{s['bytes']:>8.0f} {s['ms']:>7.1f} {s['exit_nonzero']:>7}")
    print()
    print("per-question rank of first authoritative row (None = absent):")
    for q in questions:
        cells = []
        for name in bins:
            r = next(x for x in report[name]["results"] if x["id"] == q["id"])
            cells.append(f"{name}={r['rank']}/{r['bytes']}B")
        print(f"  {q['id']:8} {q['split']:5} {'  '.join(cells)}")
    if args.confusion:
        for name, path in bins.items():
            print(f"== confidence vs top-1 ({name}, budget={args.budget}) ==")
            confusion_table(path, questions, args.budget)

    if args.json:
        with open(args.json, "w", encoding="utf-8") as fh:
            json.dump(report, fh, indent=2)
    return 0


if __name__ == "__main__":
    sys.exit(main())
