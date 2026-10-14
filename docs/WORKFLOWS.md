# Real-World Workflows: When, Where, & Why to Use ripwire

`ripwire` (`ripwire-rs`) provides static analysis, call graph navigation, and pre-commit verification gates. Rather than running commands in isolation, `ripwire-rs` is designed around 6 high-impact development and agentic workflows.

---

## 1. Orientation & Scoping (Landing Cold in a Repository)

### When & Where
- First time inspecting or onboarding into an unfamiliar codebase.
- Starting a new issue or feature request.
- Feeding high-level, budget-bounded context to an AI coding agent without blowing prompt token limits.

### Commands
```bash
# Architecture map of top 200 ranked symbols by Personalized PageRank
ripwire-rs . --top-k=200

# File-by-file structural inventory showing top 3 symbols per file
ripwire-rs . --tree

# Task-lens relevance ranking: retrieves ranked declarations relevant to a goal
ripwire-rs . --for="implement user authentication"

# Export interactive, self-contained SVG/HTML call graph to open in browser
ripwire-rs . --html=graph.html --color-by=community
```

### Benefits
- **Zero Hallucinated Dependencies**: Produces an in-context AST schema dictionary teaching agents the meaning of minified attributes (`t`, `n`, `sc`, `k`, `c`).
- **Prompt Token Savings**: Emits `shown="K"` truncation markers so LLMs know absence in a top-K slice does not mean absence in the codebase.
- **Sub-Second Speed**: Renders complete repository topologies across 36 languages in ~25 ms.

---

## 2. Navigation & Blast Radius (Safe Refactoring & Impact Analysis)

### When & Where
- Before editing or renaming a shared function or method signature.
- During incident post-mortems and bug reproduction to trace caller paths.
- Verifying the blast radius of modifying a core data model or interface.

### Commands
```bash
# Query direct 1-hop callers that depend on a symbol
ripwire-rs . --callers="Interner::len"

# Query outgoing dependencies called by a function
ripwire-rs . --callees="ingest_files"

# Transitive reachability graph: full blast radius across the whole codebase
ripwire-rs . --impact="Interner"

# Extract ego-graph around a symbol (depth 1, fanout 32)
ripwire-rs . --around="Interner::new" --around-depth=2 --around-fanout=16

# Shortest directed call path between two symbols
ripwire-rs . --path-between="main:execute"

# Minimal connecting subgraph relating 2..16 symbols
ripwire-rs . --connect="main,resolve_symbol,emit_xml"

# Intra-procedural def-use variable dataflow slicing (ARISE)
ripwire-rs . --slice="compute:x" --slice-flow=both
```

### Benefits
- **Full Transitive Impact**: Identifies downstream callers up to 16 hops deep ranked by Personalized PageRank.
- **Root-Cause Tracing**: `--path-between` and `--connect` expose the exact sequence of intermediary function calls bridging two components.
- **Side-Effect Verification**: ARISE slicing isolates exact variable lifetimes and mutation points inside a function.

---

## 3. Pre-Commit & PR Safety Gates (Zero Broken Contracts)

### When & Where
- In local Git pre-commit hooks (`.git/hooks/pre-commit`).
- Before running `git push` or opening a pull request.
- In automated CI/CD PR verification workflows.

### Commands
```bash
# Verify caller compatibility after modifying a symbol contract
ripwire-rs . --edit-check="symbol_name"

# Transitive reachability mapping to affected test suites
ripwire-rs . --affected="src/lib.rs"

# Pre-PR gate: names tests to run and untested blast radius (exits with code 4)
ripwire-rs . --test-gate

# Situational awareness report for working tree diff: blast radius, tests, co-change
ripwire-rs . --situ

# Assess whether a symbol can be safely deleted without breaking callers
ripwire-rs . --safe-delete="Interner::len"

# Measure code quality regressions against git HEAD across 10 facets (exits with code 2)
ripwire-rs . --quality-delta

# Re-rank architecture map toward git-changed files so active work floats up
ripwire-rs . --map-diff --top-k=30

# Discover untested cross-module call boundaries (integration seams)
ripwire-rs . --seams

# Generate composite PR review context bundle vs base branch
ripwire-rs . --pr-context=main

# Predict symbol merge conflicts across concurrent divergent branches
ripwire-rs . --merge-scout="branch-a,branch-b"
```

### Benefits
- **Surgical Test Execution**: Identifies the exact test files that transitively reach your changes, letting you run a 5-second test subset instead of a 20-minute test suite.
- **Contract Enforcement**: Detects parameter count additions, removals, and visibility changes that break existing callers before you commit.
- **Fowler's Shotgun Surgery Protection**: `--situ` surfaces companion files historically edited together with your diff that you may have forgotten to touch.
- **Proactive Conflict Detection**: `--merge-scout` warns of semantic conflicts across branches before rebasing.

---

## 4. Code Health & Technical Debt Auditing

### When & Where
- Sprint refactoring cycles and technical debt burn-downs.
- Architectural reviews and modularity health checks.
- Eliminating dead weight and duplicate logic.

### Commands
```bash
# Detect internal functions with zero incoming callers across the repository
ripwire-rs . --dead-code

# Rank repository files by maintenance risk (churn * cognitive complexity)
ripwire-rs . --hotspots

# Detect AST code clones and near-miss duplicates (Type-1, Type-2, Type-3)
ripwire-rs . --clones --min-tokens=40

# Mine historical Git co-change coupling to identify hidden file coupling
ripwire-rs . --cochange

# Partition call graph into cohesive modules via Louvain community detection
ripwire-rs . --communities

# Hierarchical Louvain community zoom clustering
ripwire-rs . --zoom

# Enforce architectural layering constraints from a rules file (exits 2 on violation)
ripwire-rs . --arch=rules.txt

# Export Mermaid flowchart of module dependency topology
ripwire-rs . --mermaid

# Discover symbols referenced in code but never defined (stdlib & external packages)
ripwire-rs . --external-surface

# Rank functions by readability volume and size, largest first
ripwire-rs . --biggest-first

# Detect functions accessing reachable global / file-static mutable state
ripwire-rs . --nonlocal-state

# Scan codebase for AST code smells (complexity spikes, deep nesting, dead code)
ripwire-rs . --lint
ripwire-rs . --lint-catalog
```

### Benefits
- **Data-Driven Refactoring**: Prioritizes refactoring on code hotspots where maintenance pain and churn intersect with high complexity.
- **Dead-Weight Removal**: Pinpoints zero-caller functions safely without manual grep sweeps.
- **Architectural Clarity**: Enforces strict layering boundaries via `--arch` and visualizes modular topologies with `--mermaid`.

---

## 5. Deep Inspection & Agent Context Delivery

### When & Where
- Extracting exact function implementations for LLM prompts.
- Debugging stack traces and compiler error messages.
- High-efficiency agent tooling over the Model Context Protocol (MCP).

### Commands
```bash
# Expand full source body of symbols and callee signatures in CDATA XML
ripwire-rs . --expand="Symbol::new"

# Outline control-flow skeletons of symbols (loops, branches, declarations)
ripwire-rs . --outline="Interner::intern"

# Pack source bodies of the top N highest PageRank symbols
ripwire-rs . --pack-top-n=5

# Resolve innermost symbol chain enclosing a specific FILE:LINE
ripwire-rs . --at="src/main.rs:25"

# Formally verify relationship claim between two symbols (calls, uses, defines)
ripwire-rs . --verify="calls(main,run_cli)"

# Single-call orientation task bundle tailored to a task description
ripwire-rs . --pack-task="search"

# Functional graph query DSL
ripwire-rs . --graph-query="callers(main)"

# Map compiler error / stack trace frames directly onto indexed symbols
ripwire-rs . --from-trace=error.log

# Execute up to 16 heterogeneous sub-queries in a single turn
ripwire-rs . --batch=queries.txt

# Structured JSON output for navigation queries
ripwire-rs . --callers="Interner::len" --json

# Engine self-diagnostics and repository health check
ripwire-rs . --doctor

# Pinned session gotchas and notes
ripwire-rs . --notes
ripwire-rs . --note-add="symbol: gotcha note"

# Launch native stdio Model Context Protocol (MCP) server for coding agents
ripwire-rs mcp

# Install 17 pre-packaged ripwire-rs agent skills to ~/.agents/skills (or custom path)
ripwire-rs --add-skills
```

### Benefits
- **Precision Context Bundles**: Extracts only relevant symbol bodies into CDATA blocks, conserving prompt token budgets.
- **Batching Economy**: `--batch` executes up to 16 queries in a single turn, eliminating agent tool-call round-trip latency.
- **Instant Agent Skills**: `--add-skills` automatically provisions 17 standardized agent skills and active manifest into `~/.agents/skills` for Claude Code, Codex, and Omp.

---

## 6. Continuous Integration (CI) Workflows & Automated PR Gates

### When & Where
- Automated GitHub Actions (`.github/workflows/`), GitLab CI, or Jenkins pipelines.
- Enforcing pull request quality standards and preventing regressions before merge.
- Validating 1:1 behavioral parity against upstream specifications.

### Commands & CI Pipeline Recipe
```bash
# PR Gate: names tests to run and untested blast radius (exits 4 on violation)
ripwire-rs . --test-gate

# Quality Regression Gate: checks quality regressions against base branch / HEAD (exits 2 on violation)
ripwire-rs . --quality-delta

# Upstream Parity Validation: verifies 1:1 input/output equivalence across 67 commands
make validate REPO=.

# Performance & Resource Benchmark: benchmarks speed and peak memory against upstream ripwire
make bench REPO=.

# In-Memory Cargo Pipeline Benchmark: runs statistical micro-benchmarks (cargo bench)
make bench-pipeline

#### GitHub Actions Workflow Example (`.github/workflows/ripwire-gate.yml`)
```yaml
name: Ripwire PR Safety Gates
on:
  pull_request:
    branches: [main, master]

jobs:
  ripwire-gate:
    name: Ripwire PR Gate
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Full git history for diff and quality delta comparison

      - name: Install ripwire CLI (ripwire-rs)
        run: cargo install ripwire-rs

      - name: Run Test Reachability Gate
        run: ripwire-rs . --test-gate

      - name: Run Quality Regression Delta Gate
        run: ripwire-rs . --quality-delta
```

### Benefits
- **Automated Merge Blocking**: Fails PR jobs automatically (exit code 4 on `--test-gate` or exit code 2 on `--quality-delta`) before untested changes or architectural rot merge into production branches.
- **Microsecond Gate Execution**: Executes static analysis and graph traversals in milliseconds, eliminating slow CI runner bottlenecks.
- **Zero Configuration**: Reads native git repository history, remotes, and branch merge bases automatically.
