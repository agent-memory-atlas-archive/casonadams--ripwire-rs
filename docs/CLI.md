# CLI Reference

`ripwire` provides both workflow-categorized subcommands and backward-compatible top-level flags.

## Basic Usage

```bash
# Generate ranked symbol call graph map (default 64 KB budget)
rw .

# Select ranking signal (pagerank, authority, hub)
rw . --rank-by=authority

# Set explicit streaming pack budget in bytes or tokens
rw . --pack-budget=32768
rw . --token-budget=8192
rw . --max-tokens=4096
rw . --top-k=200

# Control schema legend comment posture (compact or none)
rw . --legend=compact

# Query symbol relationships by bare name, scoped name, or file:name
rw . --callers="Interner::len"
rw . --callees="src/domain.rs:new"
rw . --impact="Type.method"

# Export self-contained interactive SVG/HTML visualizer
rw . --html=callgraph.html --color-by=community

## Detail Ladder & Progressive Expansion

**When & Where:** When drilling down from architectural map summaries to implementation details of specific functions, methods, or critical hot paths.
```bash
# Expand full source body of symbols and callee signatures
rw . --expand="Symbol::new"

# Outline control-flow skeletons of symbols (declarations, loops, branches)
rw . --outline="Interner::intern"

# Pack source bodies of the top N highest PageRank symbols
rw . --pack-top-n=5

# Orient file by file with top 3 symbols per file
rw . --tree
```
* **Benefits:** Delivers exact function bodies with zero boilerplate, eliding irrelevant lines while preserving full type signatures and callee interfaces.

---
## Pre-Commit Verification & Safety Gates

**When & Where:** Local git pre-commit hooks, pre-push checks, or automated CI/CD pull request verification.
```bash
# Verify caller compatibility after editing a symbol contract
rw . --edit-check="symbol_name"

# Transitive reachability mapping to affected test suites
rw . --affected="src/lib.rs"

# Name tests to run and untested blast radius (exit 4 if non-empty obligations)
rw . --test-gate

# Situational awareness report for working tree diff: blast radius, tests, co-change
rw . --situ

# Safe delete assessment joining callers, blast radius, tests, and history
rw . --safe-delete="Interner::len"

# Show non-test symbols reached by a test suite
rw . --exercises="tests/test_cli_and_mcp.rs"

# Snapshot code quality baseline at current commit
rw . --quality-baseline

# Check for quality regressions against baseline or git HEAD (exits 2 on failure)
rw . --quality-delta

# Re-rank map toward git-changed files so recent work floats up
rw . --map-diff --top-k=30

# Untested cross-module call boundaries (integration seams)
rw . --seams

# Composite PR review context bundle vs base ref
rw . --pr-context=main

# Predict symbol merge conflicts across concurrent branches
rw . --merge-scout="branch-a,branch-b"
```
* **Benefits:** Catches breaking signature changes, eliminates test suite bloat by running only affected tests, and stops architectural decay before merging.

---
## Architecture & Code Intelligence

**When & Where:** Sprint refactoring, architectural reviews, or technical debt auditing.
```bash
# Detect internal functions with zero incoming callers across the tree
rw . --dead-code

# Partition call graph into cohesive modules (Louvain community detection)
rw . --communities

# Extract ego-graph around a symbol (depth 1, fanout 32)
rw . --around="Interner::new" --around-depth=2 --around-fanout=16

# Discover symbols referenced but never defined (stdlib & external dependencies)
rw . --external-surface

# File-to-file dependency graph with instability and afferent coupling
rw . --deps

# Detect AST code clones and near-miss duplicates (Type-1, Type-2, Type-3)
rw . --clones --min-tokens=40

# Mine historical git co-change coupling (Fowler's Shotgun Surgery)
rw . --cochange

# Compute maintenance hotspots (churn * cognitive complexity)
rw . --hotspots

# Enforce architectural layering rules from text file (exits 2 on violation)
rw . --arch=rules.txt

# Export Mermaid flowchart of module call dependencies
rw . --mermaid

# Hierarchical Louvain community clustering
rw . --zoom

# Rank functions by readability volume and size, largest first
rw . --biggest-first

# Detect functions accessing reachable global / file-static mutable state
rw . --nonlocal-state

# Scan codebase for AST code smells (complexity spikes, deep nesting, dead functions)
rw . --lint

# Print built-in AST code smell catalog
rw . --lint-catalog

# Compute ARISE variable def-use dataflow slice (forward/backward)
rw . --slice="compute:x" --slice-flow=fwd
```
* **Benefits:** Focuses engineering attention directly on defect-prone hotspots, hidden file coupling, and dead code with mathematical precision.

---
## Task Retrieval & Batching

**When & Where:** Debugging compiler errors/stack traces, precision prompting for AI agents, or multi-query workflows.
```bash
# Find best-in-class existing implementation of kind to imitate
rw . --exemplar="parser"

# Show one interface contract and all types implementing it
rw . --lego="ToXmlString"

# Search repository markdown documentation for relevant plans and notes
rw . --recall="PageRank"

# Map stack trace / compiler error lines onto indexed symbols
rw . --from-trace=error.log

# Emit JSON instead of XML for navigation queries
rw . --callers="Interner::len" --json

# Execute up to 16 heterogeneous sub-queries in a single turn
rw . --batch=queries.txt

# Resolve innermost symbol chain enclosing FILE:LINE
rw . --at="src/main.rs:25"

# Formally verify relationship claim (calls, uses, defines, contains)
rw . --verify="calls(main,run_cli)"

# Single-call orientation task bundle under budget
rw . --pack-task="search"

# Functional graph query DSL
rw . --graph-query="callers(main)"

# Self-check diagnostics
rw . --doctor

# Pinned session gotchas and notes
rw . --notes
rw . --note-add="symbol: gotcha note"

# Install 18 pre-packaged ripwire agent skills to ~/.agents/skills (or custom path)
rw --add-skills
```
* **Benefits:** Minimal prompt token consumption with maximum semantic density for coding agents.

---
## Workflow Subcommands

- `rw orient [PATH]`: Codebase orientation and structural summaries.
- `rw navigate [PATH] --symbol <SYM> [--callers|--callees|--impact]`: Symbol neighborhood analysis.
- `rw gate [PATH] [--edit-check|--affected|--quality-delta|--test-gate]`: Verification safety gates.
- `rw metrics [PATH] [--hotspots|--complexity|--clones]`: Code health metrics.
- `rw slice [PATH] --symbol <SYM> --var <VAR>`: Intra-procedural slicing.
- `rw mcp`: Launch native JSON-RPC Model Context Protocol (MCP) server over stdio.
- `rw skills [DIR]`: Install and manage 18 ripwire agent skills in `~/.agents/skills`.
