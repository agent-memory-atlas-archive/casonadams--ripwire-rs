# Real-World Workflows: When, Where, & Why to Use ripwire

`ripwire` (`rw`) provides static analysis, call graph navigation, and pre-commit verification gates. Rather than running commands in isolation, `rw` is designed around 6 high-impact development and agentic workflows.

---

## 1. Orientation & Scoping (Landing Cold in a Repository)

### When & Where
- First time inspecting or onboarding into an unfamiliar codebase.
- Starting a new issue or feature request.
- Feeding high-level, budget-bounded context to an AI coding agent without blowing prompt token limits.

### Commands
```bash
# Architecture map of top 200 ranked symbols by Personalized PageRank
rw . --top-k=200

# File-by-file structural inventory showing top 3 symbols per file
rw . --tree

# Task-lens relevance ranking: retrieves ranked declarations relevant to a goal
rw . --for="implement user authentication"

# Export interactive, self-contained SVG/HTML call graph to open in browser
rw . --html=graph.html --color-by=community
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
rw . --callers="Interner::len"

# Query outgoing dependencies called by a function
rw . --callees="ingest_files"

# Transitive reachability graph: full blast radius across the whole codebase
rw . --impact="Interner"

# Extract ego-graph around a symbol (depth 1, fanout 32)
rw . --around="Interner::new" --around-depth=2 --around-fanout=16

# Shortest directed call path between two symbols
rw . --path-between="main:execute"

# Minimal connecting subgraph relating 2..16 symbols
rw . --connect="main,resolve_symbol,emit_xml"

# Intra-procedural def-use variable dataflow slicing (ARISE)
rw . --slice="compute:x" --slice-flow=both
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
rw . --edit-check="symbol_name"

# Transitive reachability mapping to affected test suites
rw . --affected="src/lib.rs"

# Pre-PR gate: names tests to run and untested blast radius (exits with code 4)
rw . --test-gate

# Situational awareness report for working tree diff: blast radius, tests, co-change
rw . --situ

# Assess whether a symbol can be safely deleted without breaking callers
rw . --safe-delete="Interner::len"

# Measure code quality regressions against git HEAD across 10 facets (exits with code 2)
rw . --quality-delta

# Re-rank architecture map toward git-changed files so active work floats up
rw . --map-diff --top-k=30

# Discover untested cross-module call boundaries (integration seams)
rw . --seams

# Generate composite PR review context bundle vs base branch
rw . --pr-context=main

# Predict symbol merge conflicts across concurrent divergent branches
rw . --merge-scout="branch-a,branch-b"
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
rw . --dead-code

# Rank repository files by maintenance risk (churn * cognitive complexity)
rw . --hotspots

# Detect AST code clones and near-miss duplicates (Type-1, Type-2, Type-3)
rw . --clones --min-tokens=40

# Mine historical Git co-change coupling to identify hidden file coupling
rw . --cochange

# Partition call graph into cohesive modules via Louvain community detection
rw . --communities

# Hierarchical Louvain community zoom clustering
rw . --zoom

# Enforce architectural layering constraints from a rules file (exits 2 on violation)
rw . --arch=rules.txt

# Export Mermaid flowchart of module dependency topology
rw . --mermaid

# Discover symbols referenced in code but never defined (stdlib & external packages)
rw . --external-surface

# Rank functions by readability volume and size, largest first
rw . --biggest-first

# Detect functions accessing reachable global / file-static mutable state
rw . --nonlocal-state

# Scan codebase for AST code smells (complexity spikes, deep nesting, dead code)
rw . --lint
rw . --lint-catalog
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
rw . --expand="Symbol::new"

# Outline control-flow skeletons of symbols (loops, branches, declarations)
rw . --outline="Interner::intern"

# Pack source bodies of the top N highest PageRank symbols
rw . --pack-top-n=5

# Resolve innermost symbol chain enclosing a specific FILE:LINE
rw . --at="src/main.rs:25"

# Formally verify relationship claim between two symbols (calls, uses, defines)
rw . --verify="calls(main,run_cli)"

# Single-call orientation task bundle tailored to a task description
rw . --pack-task="search"

# Functional graph query DSL
rw . --graph-query="callers(main)"

# Map compiler error / stack trace frames directly onto indexed symbols
rw . --from-trace=error.log

# Execute up to 16 heterogeneous sub-queries in a single turn
rw . --batch=queries.txt

# Structured JSON output for navigation queries
rw . --callers="Interner::len" --json

# Engine self-diagnostics and repository health check
rw . --doctor

# Pinned session gotchas and notes
rw . --notes
rw . --note-add="symbol: gotcha note"

# Launch native stdio Model Context Protocol (MCP) server for coding agents
rw mcp

# Install 18 pre-packaged ripwire agent skills to ~/.agents/skills (or custom path)
rw --add-skills
```

### Benefits
- **Precision Context Bundles**: Extracts only relevant symbol bodies into CDATA blocks, conserving prompt token budgets.
- **Batching Economy**: `--batch` executes up to 16 queries in a single turn, eliminating agent tool-call round-trip latency.
- **Instant Agent Skills**: `--add-skills` automatically provisions 18 standardized agent skills and active manifest into `~/.agents/skills` for Claude Code, Codex, and Omp.

---

## 6. Continuous Integration (CI) Workflows & Automated PR Gates

### When & Where
- Automated GitHub Actions (`.github/workflows/`), GitLab CI, or Jenkins pipelines.
- Enforcing pull request quality standards and preventing regressions before merge.
- Validating 1:1 behavioral parity against upstream specifications.

### Commands & CI Pipeline Recipe
```bash
# PR Gate: names tests to run and untested blast radius (exits 4 on violation)
rw . --test-gate

# Quality Regression Gate: checks quality regressions against base branch / HEAD (exits 2 on violation)
rw . --quality-delta

# Upstream Parity Validation: verifies 1:1 input/output equivalence across 36 commands
make validate REPO=.
```

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

      - name: Install ripwire CLI (rw)
        run: cargo install ripwire

      - name: Run Test Reachability Gate
        run: rw . --test-gate

      - name: Run Quality Regression Delta Gate
        run: rw . --quality-delta
```

### Benefits
- **Automated Merge Blocking**: Fails PR jobs automatically (exit code 4 on `--test-gate` or exit code 2 on `--quality-delta`) before untested changes or architectural rot merge into production branches.
- **Microsecond Gate Execution**: Executes static analysis and graph traversals in milliseconds, eliminating slow CI runner bottlenecks.
- **Zero Configuration**: Reads native git repository history, remotes, and branch merge bases automatically.
