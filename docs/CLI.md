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

# Control schema legend comment posture (compact or none)
ripwire-rs . --legend=compact

# Query symbol relationships by bare name, scoped name, or file:name
ripwire-rs . --callers="Interner::len"
ripwire-rs . --callees="src/domain.rs:new"
ripwire-rs . --impact="Type.method"

# Export self-contained interactive SVG/HTML visualizer
ripwire-rs . --html=callgraph.html --color-by=community

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

# Snapshot code quality baseline at current commit
ripwire-rs . --quality-baseline

# Check for quality regressions against baseline or git HEAD (exits 2 on failure)
ripwire-rs . --quality-delta

# Re-rank map toward git-changed files so recent work floats up
ripwire-rs . --map-diff --top-k=30

# Untested cross-module call boundaries (integration seams)
ripwire-rs . --seams

# Composite PR review context bundle vs base ref
ripwire-rs . --pr-context=main

# Predict symbol merge conflicts across concurrent branches
ripwire-rs . --merge-scout="branch-a,branch-b"
```
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

# Hierarchical Louvain community clustering
ripwire-rs . --zoom

# Rank functions by readability volume and size, largest first
ripwire-rs . --biggest-first

# Detect functions accessing reachable global / file-static mutable state
ripwire-rs . --nonlocal-state

# Scan codebase for AST code smells (complexity spikes, deep nesting, dead functions)
ripwire-rs . --lint

# Print built-in AST code smell catalog
ripwire-rs . --lint-catalog

# Compute ARISE variable def-use dataflow slice (forward/backward)
ripwire-rs . --slice="compute:x" --slice-flow=fwd
```
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

# Install 18 pre-packaged ripwire agent skills to ~/.agents/skills (or custom path)
ripwire-rs --add-skills
```
* **Benefits:** Minimal prompt token consumption with maximum semantic density for coding agents.

---
## Workflow Subcommands

- `ripwire-rs orient [PATH]`: Codebase orientation and structural summaries.
- `ripwire-rs navigate [PATH] --symbol <SYM> [--callers|--callees|--impact]`: Symbol neighborhood analysis.
- `ripwire-rs gate [PATH] [--edit-check|--affected|--quality-delta|--test-gate]`: Verification safety gates.
- `ripwire-rs metrics [PATH] [--hotspots|--complexity|--clones]`: Code health metrics.
- `ripwire-rs slice [PATH] --symbol <SYM> --var <VAR>`: Intra-procedural slicing.
- `ripwire-rs mcp`: Launch native JSON-RPC Model Context Protocol (MCP) server over stdio.
- `ripwire-rs skills [DIR]`: Install and manage 18 ripwire-rs agent skills in `~/.agents/skills`.
