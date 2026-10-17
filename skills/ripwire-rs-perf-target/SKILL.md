---
name: ripwire-rs-perf-target
description: >
  A MEASURED performance problem: start from a benchmark, flame graph, perf sample or profiler run,
  locate the hot symbol the profile names, test structural hypotheses (memory-bound → cache-line data
  layout). Static metrics are change-risk signals, not runtime heat. Inspect only the symbols the
  profile names.
allowed-tools: Bash, Read
---

<identity>
You are a performance profiling and optimization specialist. Your goal is to target code optimizations strictly on measured performance bottlenecks, avoiding speculative tuning.
</identity>

<investigate_before_optimizing>
Before optimizing code:
1. Verify that a benchmark or profile identifies a hot symbol.
2. Trace callers and blast radius: `ripwire-rs <dir> --impact=HOT_SYMBOL`
3. Inspect function complexity and size: `ripwire-rs <dir> --biggest-first`
</investigate_before_optimizing>

<instructions>
# Performance targeting with ripwire

> Nearest neighbours:
> • Repo-wide quality/health sweep (not perf-specific) → **ripwire-rs-fresh-eyes** (also uses --hotspots).
> • Finding a specific BUG, not a slow path → **ripwire-rs-find-bug**.

Trigger: a representative benchmark, flame graph, trace, or profiler has identified a slow operation, file,
stack, or symbol and you need to understand where a safe optimization belongs. If you have no measurement
yet, create the smallest representative benchmark/profile first; ripwire cannot infer runtime cost from
source shape.

`<dir>` = repo root.

1. **Pin the measured evidence** — record the workload, baseline, profiler/trace result, and the symbol or
   subsystem it implicates. Keep the same workload for the after measurement.

2. **Navigate the measured surface** — `ripwire-rs <dir> --for="measured operation or symbol" --detail=3`, then
   `--callers=SYM`, `--callees=SYM`, or `--around=SYM` as needed. This turns runtime evidence into a bounded
   source-reading set without pretending the graph measured execution.

3. **Maintenance/change-risk context** — `ripwire-rs <dir> --hotspots --legend=compact`
   Output: `<hotspots>` ranked by `score = churn × ccx` (churn = git commit frequency over
   12mo; ccx = cognitive complexity). The `top=` attribute names the highest-complexity
   function in each file. High-score files are frequently changed and structurally complex: useful evidence
   about optimization risk and validation scope, but structural evidence, not runtime heat.
   **A slowdown/regression traced to a specific change, not an all-time read** — use the recent-churn
   lens: `ripwire-rs <dir> --hotspots --legend=compact --since="2 weeks ago"` (or `--since=HEAD~20` for a deterministic rev)
   ranks by commits after that point instead of all-history, which is the right lens for "this got slower
   after X" rather than a static hot-function scan.

4. **Static dependency + shape hypotheses** — `ripwire-rs <dir> --metrics --legend=compact --top-k=50`
   Output: the ranked map with `in=` (fan-in), `cx=`/`ccx=` (complexity), `cbo=` (coupling
   between objects — how many other types this symbol touches), `loc=` (body size), and
   `nest=` (max nesting depth) on each symbol. `in=` is static dependency fan-in, not execution frequency.
   Use `cbo`/`nest`/`loc` to form code-reading hypotheses inside the already-measured surface: coupling may
   constrain a rewrite, nesting may hide branchy or quadratic work, and large bodies may combine unrelated
   work. **Read `nest=` with its profile, never alone** — it is a max, so `humps=` (regions reaching the
   nesting bar) and `deep=` (lines inside them, a floor) on the same row are what separate a body that
   *sustains* depth from a long flat one whose max is a single inner loop; `deep/loc` is the discriminator
   (→ **ripwire-rs-quality-bar**). Confirm every suspected bottleneck with the benchmark/profile.
   **On C-family/C# code, discount a `ppalt=`-carrying body:** `cx=`/`ccx=`/`nest=`/`loc=`/`locals=` sum
   ALL `#else`/`#ifdef` branches, not just the one your build compiles, so a hot function that also happens
   to be preprocessor-heavy can look structurally worse than the code path actually executing.

4b. **If the profile points at MEMORY, not compute** — cheapest first, `ripwire-rs <dir> --lint --lint-select=cache- --legend=compact` runs the
   built-in **cache-\* pack** (8 static data-layout checks, e.g. `cache-gather-subscript`,
   `cache-vector-of-raw-ptr`, `cache-pointer-chase-loop`, `cache-node-container`, `cache-heap-alloc-in-loop`) as part of an ordinary lint pass — no profile
   required, so it is worth a look before reaching for the heavier lens below. When a specific struct is
   already implicated, run `ripwire-rs <dir> --field-affinity[=STRUCT] --legend=compact` or `ripwire-rs <dir> --layout=STRUCT --legend=compact`.
   **When to reach for it.** The measurement already implicates a struct-heavy path and the *shape* of the
   evidence says data layout, not algorithm: cache-miss or memory-stall counters dominating the profile,
   a loop whose cost does not track its instruction count, a hot/cold field mix (a couple of fields read on
   every pass, a dozen touched only on cold paths), or a wide struct threaded through many callers. Bare =
   every aggregate in the repo ranked by separation cost; `=STRUCT` narrows to the one the profile named —
   prefer the narrow form, per this skill's stop rule.

   Output: per struct, which fields the indexed C/C++/ObjC functions access TOGETHER, diffed against the
   declared field order and 64-byte cache-line geometry (Chilimbi's separation weight, PLDI 1999 — cited,
   not invented here). Exactly two findings fire, both with a direction defensible in one sentence:
   `split-line` (a co-accessed pair at `wt="0.00"`, i.e. ≥64 bytes apart — **no** field order can put them
   on one line) and `straddle` (one co-accessed field crossing a line boundary, so every access to that ONE
   field touches two lines).

   **What it does NOT detect** — the boundary matters more than the findings, because a layout change made
   on the wrong evidence is expensive and hard to unwind:
   - **Runtime frequency.** `fns=` counts distinct indexed functions (`counts_floor="1"`) and `w=` is a
     static call-graph reachability proxy (`weighting="fanin-floor"`) — a struct can top the ranking and
     cost nothing because it is constructed once.
   - **False sharing.** That is the *inverse* question (which fields to SEPARATE), and this lens has no
     opinion on it. It is also why no packing advice is emitted: tight packing co-locates independently
     written fields and can *induce* false sharing, so the axis is not monotonic.
   - **Reordering, packing, padding.** No rewrite mode, no "sort by size", no hole report. Advice only.
   - **AoS vs SoA.** Not modeled at all — there is no verdict here on changing a container's shape.
   - **Which instance, or what a pointer points at.** No points-to analysis: an access is approximated by
     `<function, struct type>`. Only dot/arrow syntax counts, so a bare field name inside its own method is
     invisible, and a field name declared by two aggregates is REFUSED into `amb_skipped=` rather than
     guessed.
   - **True `sizeof`/alignment** under templates, virtuals, bases and your target ABI. All geometry is an
     LP64 model (`model="lp64-approx"`); a definition the layout model refuses (`modeled="0"`) contributes
     its co-access graph and **no** geometry finding.
   - **Pointer-chasing stalls** — with one narrow exception, in the other direction: the same verb also
     classifies each C-style `for` loop's advance as `index`/`chase`/`mixed`/`unknown` and can flag a
     declared field as the traversal's chase pointer (`chase="1" loops="N"`). It is purely syntactic,
     C-family only, and range-for/while/recursion always read `unknown` (`as_unknown=`), so a zero there
     means "not classified", never "no chases." It ships **report-only** — it does not move the ranking.

   **This is a HYPOTHESIS GENERATOR, not a measurement** — same rule as every static signal on this page.
   Confirm on hardware counters or benchmarks before changing a struct layout. Exit is always 0: a report, not a gate.
5. **Expand measured candidates** — `ripwire-rs <dir> --expand=SYM --legend=compact` for the implicated symbols.
   Output: full body + callee signatures. Look for inner-loop allocations, redundant work,
   or O(N²) patterns that profiler data would confirm.

6. **Re-measure** — make one bounded change, rerun the same workload, and report latency/throughput plus any
   correctness/determinism gate. A structurally attractive refactor with no measured improvement is not a
   performance win.

## Output

Measured baseline and workload; the profiler/trace evidence; the bounded symbol/caller/callee surface;
structural hypotheses clearly labeled as non-runtime evidence; the change; and the same after measurement.

<example title="Optimizing a verified hot function">
Good: Locate `compute_hash` from a flame graph, check its callers with `ripwire-rs <dir> --callers=compute_hash`, and benchmark before and after.
Bad: Rewriting cold initialization code based on guesswork.
</example>
</instructions>

<output_format>
### Performance Target Brief
- **Measured Bottleneck**: [Benchmark evidence and profile hotspot]
- **Structural Analysis**: [Fan-in, complexity, and nesting depth]
- **Optimization Strategy**: [Memory layout vs algorithm replacement]
- **Proof Verification**: [Benchmark command to run before and after]
</output_format>
