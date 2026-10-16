# CLI Reference

`ripwire` provides both workflow-categorized subcommands and backward-compatible top-level flags.

## Basic Usage

```bash
# Generate ranked symbol call graph map (default 64 KB budget)
ripwire-rs .

# Select ranking signal (pagerank, authority, hub)
ripwire-rs . --rank-by=authority

# Set explicit streaming pack budget in bytes or tokens
ripwire-rs . --pack-budget=32768
ripwire-rs . --token-budget=8192
ripwire-rs . --max-tokens=4096
ripwire-rs . --top-k=200

# Control schema legend comment posture (compact or none) and formatting (--pretty, --json)
ripwire-rs . --legend=compact --pretty
ripwire-rs . --top-k=20 --json
# Query symbol relationships by bare name, scoped name, or file:name
ripwire-rs . --callers="Interner::len"
ripwire-rs . --callees="src/domain.rs:new"
ripwire-rs . --impact="Type.method"

# Query static use-sites (call, read, write, import, extends)
ripwire-rs . --uses="Interner"

# Cross-language embedded SQL table lineage (zero flags required)
ripwire-rs . --callers="carrier_sim_swap"
ripwire-rs . --uses="carrier_sim_swap"
ripwire-rs . --impact="carrier_sim_swap"
# Export self-contained interactive SVG/HTML visualizer
ripwire-rs . --html=callgraph.html --color-by=community

---
## Language-Agnostic Embedded SQL Lineage

**When & Where:** When auditing database schema changes, tracking table mutations, refactoring SQL migrations, or understanding which backend services and endpoints read from or write to a database table.

SQL queries come **for free automatically** during standard AST ingestion—no new flag is required.

Whenever `ripwire-rs` indexes a repository, any string literal in any host language (Dart, Rust, Python, TypeScript, Go, Java, C#, etc.) containing SQL DML (`SELECT ... FROM`, `JOIN`, `INSERT INTO`, `UPDATE`, `DELETE FROM`) automatically emits `RefRole::Read` and `RefRole::Write` references attributed to the enclosing function or method, and resolves them directly to `.sql` table/view symbols in the call graph.

```bash
# List all functions/methods across the codebase that query or mutate carrier_sim_swap
ripwire-rs . --callers="carrier_sim_swap"

# List every read and write site with exact file:line and enclosing symbol
ripwire-rs . --uses="carrier_sim_swap"

# Calculate transitive blast radius from a table to all downstream callers and tests
ripwire-rs . --impact="carrier_sim_swap"

# Safety gates automatically trace from .sql migration files through to application tests
ripwire-rs . --test-gate
ripwire-rs . --situ
```
* **Benefits:** Eliminates the blind spot between database schemas and application code, turning string-embedded SQL queries into typed graph edges without runtime dependencies or database connections.

## Detail Ladder & Progressive Expansion

**When & Where:** When drilling down from architectural map summaries to implementation details of specific functions, methods, or critical hot paths.
```bash
# Expand full source body of symbols and callee signatures
ripwire-rs . --expand="Symbol::new"

# Outline control-flow skeletons of symbols (declarations, loops, branches)
ripwire-rs . --outline="Interner::intern"

# Pack source bodies of the top N highest PageRank symbols
ripwire-rs . --pack-top-n=5

# Orient file by file with top 3 symbols per file
ripwire-rs . --tree
```
* **Benefits:** Delivers exact function bodies with zero boilerplate, eliding irrelevant lines while preserving full type signatures and callee interfaces.

---
## Pre-Commit Verification & Safety Gates

**When & Where:** Local git pre-commit hooks, pre-push checks, or automated CI/CD pull request verification.
```bash
# Verify caller compatibility after editing a symbol contract
ripwire-rs . --edit-check="symbol_name"

# Transitive reachability mapping to affected test suites
ripwire-rs . --affected="src/lib.rs"

# Name tests to run and untested blast radius (exit 4 if non-empty obligations)
ripwire-rs . --test-gate

# Situational awareness report for working tree diff: blast radius, tests, co-change
ripwire-rs . --situ

# Safe delete assessment joining callers, blast radius, tests, and history
ripwire-rs . --safe-delete="Interner::len"

# Show non-test symbols reached by a test suite
ripwire-rs . --exercises="tests/test_cli_and_mcp.rs"

# Snapshot code quality baseline at current commit (refuses on a dirty tree that already gates)
ripwire-rs . --quality-baseline

# Pin anyway; the absorbed gating count is stamped and reported as baseline_absorbed=
ripwire-rs . --quality-baseline --allow-dirty

# Check for quality regressions against baseline or git HEAD (exits 2 on failure)
ripwire-rs . --quality-delta

# Print stale ack rows that a regression-free delta collapses to stale_by_kind=
ripwire-rs . --quality-delta --show-stale

# Re-rank map toward git-changed files so recent work floats up
ripwire-rs . --map-diff --top-k=30

# Untested cross-module call boundaries (integration seams)
ripwire-rs . --seams

# Composite PR review context bundle vs base ref
ripwire-rs . --pr-context=main

# Predict symbol merge conflicts across concurrent branches
ripwire-rs . --merge-scout="branch-a,branch-b"

# Delta Maintainability Model (DMM) scalar in [0,1] measuring healthy code churn
ripwire-rs . --dmm

# Pre-hoc parallel branch collision forecasting before writing code
ripwire-rs . --plan-lanes=3 --task="refactor parser"

# Security audit of agent skill files for exfiltration or command injection
ripwire-rs . --scan-skills

# Pin architectural baseline sidecar or update it with new violations
ripwire-rs . --baseline
ripwire-rs . --baseline-update

# Pin quality census snapshot
ripwire-rs . --pin-census

# Partition quality-delta by author/team ownership scope
ripwire-rs . --quality-delta --scope="core"

# Cluster co-changing files into multi-file change groups and set recurrence threshold
ripwire-rs . --cochange --cochange-groups --cochange-recur=3

# Partition codebase into cohesive modules
ripwire-rs . --partition=5

# Execute transactional versioned JSON edit plan (with --dry-run validation)
ripwire-rs . --dry-run --edit-plan=plan.json
ripwire-rs . --edit-target-file="src/domain.rs" --no-post-check --allow-remote-edits
* **Benefits:** Catches breaking signature changes, eliminates test suite bloat by running only affected tests, and stops architectural decay before merging.

---
## Architecture & Code Intelligence

**When & Where:** Sprint refactoring, architectural reviews, or technical debt auditing.
```bash
# Detect internal functions with zero incoming callers across the tree
ripwire-rs . --dead-code

# Partition call graph into cohesive modules (Louvain community detection)
ripwire-rs . --communities

# Extract ego-graph around a symbol (depth 1, fanout 32)
ripwire-rs . --around="Interner::new" --around-depth=2 --around-fanout=16

# Discover symbols referenced but never defined (stdlib & external dependencies)
ripwire-rs . --external-surface

# File-to-file dependency graph with instability and afferent coupling
ripwire-rs . --deps

# Detect AST code clones and near-miss duplicates (Type-1, Type-2, Type-3)
ripwire-rs . --clones --min-tokens=40

# Mine historical git co-change coupling (Fowler's Shotgun Surgery)
ripwire-rs . --cochange

# Compute maintenance hotspots (churn * cognitive complexity)
ripwire-rs . --hotspots

# Enforce architectural layering rules from text file (exits 2 on violation)
ripwire-rs . --arch=rules.txt

# Export Mermaid flowchart of module call dependencies
ripwire-rs . --mermaid

# Multi-level Louvain module hierarchy (computed levels=, top 2 printed)
ripwire-rs . --zoom

# Print every hierarchy level
ripwire-rs . --zoom --zoom-levels=0

# Rank functions by readability volume and size, largest first
ripwire-rs . --biggest-first

# Detect functions accessing reachable global / file-static mutable state
ripwire-rs . --nonlocal-state

# Scan codebase for AST code smells (XML or SARIF 2.1.0 JSON output)
ripwire-rs . --lint
ripwire-rs . --lint --sarif
# Print built-in AST code smell catalog
ripwire-rs . --lint-catalog

# Compute ARISE variable def-use dataflow slice (forward/backward)
ripwire-rs . --slice="compute:x" --slice-flow=fwd

# Annotate every symbol with fan-in, fan-out, complexity, and change-amplification
ripwire-rs . --metrics

# Entire quality panel in one report across 6 evidence families (strict|default|lenient)
ripwire-rs . --quality-panel=default

# Show struct memory layout with computed LP64 field offsets and alignment padding
ripwire-rs . --layout="Symbol"

# Cache-conscious struct field affinity analysis against 64-byte lines
ripwire-rs . --field-affinity="Symbol"

# Local-reasoning lens measuring share of context outside a unit's file
ripwire-rs . --context-ratio

# Four orthogonal evidence families joined, ranked by distinct families fired
ripwire-rs . --ensemble

# Doc comment to identifier coherence verification
ripwire-rs . --comment-coherence

# Set maximum hop depth for intra-procedural slicing (default: 8)
ripwire-rs . --slice="compute:x" --slice-flow=fwd --slice-depth=12

# Select or ignore specific AST lint rules
ripwire-rs . --lint --lint-select="complexity-spike,deep-nesting"
ripwire-rs . --lint --lint-ignore="magic-number"

# Calibrate naming rules against git rename history
ripwire-rs . --naming-consistency --naming-calibration

# Generate executive markdown architecture report
ripwire-rs . --report

# Emit full schema legend dictionary
ripwire-rs . --legend-dict

# Disable stable dependency violation section in --deps
ripwire-rs . --deps --no-stable
* **Benefits:** Focuses engineering attention directly on defect-prone hotspots, hidden file coupling, and dead code with mathematical precision.

---
## Task Retrieval & Batching

**When & Where:** Debugging compiler errors/stack traces, precision prompting for AI agents, or multi-query workflows.
```bash
# Find best-in-class existing implementation of kind to imitate
ripwire-rs . --exemplar="parser"

# Show one interface contract and all types implementing it
ripwire-rs . --lego="ToXmlString"

# Search repository markdown documentation for relevant plans and notes
ripwire-rs . --recall="PageRank"

# Map stack trace / compiler error lines onto indexed symbols
ripwire-rs . --from-trace=error.log

# Emit JSON instead of XML for navigation queries
ripwire-rs . --callers="Interner::len" --json

# Execute up to 16 heterogeneous sub-queries in a single turn
ripwire-rs . --batch=queries.txt

# Resolve innermost symbol chain enclosing FILE:LINE
ripwire-rs . --at="src/main.rs:25"

# Formally verify relationship claim (calls, uses, defines, contains)
ripwire-rs . --verify="calls(main,run_cli)"

# Single-call orientation task bundle under budget
ripwire-rs . --pack-task="search"

# Functional graph query DSL
ripwire-rs . --graph-query="callers(main)"

# Self-check diagnostics
ripwire-rs . --doctor

# Pinned session gotchas and notes
ripwire-rs . --notes
ripwire-rs . --note-add="symbol: gotcha note"

# Importance-weighted detail: full bodies for top N symbols, signatures for rest
ripwire-rs . --for="parser" --detail=5

# Cut returned set at the relevance cliff (Adaptive-k)
ripwire-rs . --for="parser" --adaptive

# Print declaration skeletons with bodies elided (~80% token savings)
ripwire-rs . --for="parser" --pack-signatures
# Find which local branch defines or mentions a symbol (HEAD first, with git history oracle)
ripwire-rs . --whereis="Symbol" --with-history --whereis-full-text --whereis-listing=defs

# Sweep local branches for divergent authored work vs merge-base with HEAD
ripwire-rs . --stray-content

# Self-evaluations: co-change recall and known-item retrieval benchmarks
ripwire-rs . --eval
ripwire-rs . --eval-retrieval
# Install 17 pre-packaged ripwire-rs agent skills to ~/.agents/skills (or custom path)
ripwire-rs --add-skills

# Restrict grep to path substring, enclosing symbol scope, or include context lines
ripwire-rs . --grep="Interner" --grep-in="src/" --grep-scope="domain" --grep-context=2
# Conjunction, negation, and enclosing scope filters for structural search
ripwire-rs . --grep="cache" --and="payload" --not="deprecated" --in="src/cache.rs"

# Emit compact refetch handles and re-expand them
ripwire-rs . --for="parser" --handles
ripwire-rs . --refetch="h:abc123"

# Recommend the best ripwire verb for a natural-language task
ripwire-rs . --help-task="find callers of function and test blast radius"

# Crawler filtering: exclude patterns, ignore gitignore, ignore test files, file size limits
ripwire-rs . --exclude="vendor/*" --no-ignore --ignore-tests --max-file-size=500000

# Compress output with compact token dialect or export full index
ripwire-rs . --compress
ripwire-rs . --export="index.json" --index-out="index.bin"

# Annotate symbols with runtime profile weights or trace execution
ripwire-rs . --with-profile="flamegraph.perf"
ripwire-rs . --run-trace="cargo test" --run-timeout=30

# Additional benchmark evaluation modes: mined git commits, skills, and stray branches
ripwire-rs . --eval-mined
ripwire-rs . --eval-skills
ripwire-rs . --eval-stray
```
* **Benefits:** Minimal prompt token consumption with maximum semantic density for coding agents.

---
## Workflow Subcommands

- `ripwire-rs orient [PATH] [--for <TASK>]`: Codebase orientation and task-ranked summaries.
- `ripwire-rs navigate [PATH] --symbol <SYM> [--callers|--callees|--impact]`: Symbol neighborhood analysis.
- `ripwire-rs gate [PATH] [--edit-check|--affected|--quality-delta|--test-gate]`: Verification safety gates.
- `ripwire-rs metrics [PATH] [--hotspots|--complexity|--clones]`: Code health metrics.
- `ripwire-rs slice [PATH] <SYM:VAR> [--flow <fwd|back|both>]`: Intra-procedural slicing.
- `ripwire-rs mcp [PATH]`: Launch native JSON-RPC Model Context Protocol (MCP) server over stdio.
- `ripwire-rs [PATH] --lsp`: Launch stdio JSON-RPC Language Server Protocol (LSP) server bridge.
- `ripwire-rs wrap <AGENT>`: Generate agent integration recipe (`claude`, `cursor`, `codex`, `windsurf`, `gemini`, `opencode`, `aider`).
- `ripwire-rs [PATH] [--cache <PATH>] [--no-cache]`: Persistent binary on-disk caching in TMPDIR (or custom path) for sub-second repeat queries.
- `make bench [REPO=<PATH>]`: Multi-metric performance and resident set size (RSS) benchmark against C++ `ripwire`.
- `make bench-pipeline`: Native in-memory Cargo micro-benchmarks (`cargo bench --bench bench_pipeline`).
