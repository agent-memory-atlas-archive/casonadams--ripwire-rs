# ripwire-rs

High-throughput agent context router and deterministic static analysis engine.

Rust port and rewrite of [redhat-et/ripwire](https://github.com/redhat-et/ripwire), originally created and authored by David Brewster at Red Hat Emerging Technologies.

`ripwire-rs` analyzes source code across 36 languages to construct precise call graphs, calculate symbol impact via PageRank and HITS, mine co-change patterns, slice intra-procedural dataflow, and enforce pre-commit quality gates. It runs as a standalone CLI or as a Model Context Protocol (MCP) server over stdio for AI coding agents.

---

## Quickstart

```bash
# 1. Orientation: generate ranked architecture map (top 200 symbols)
ripwire-rs . --top-k=200

# 2. Navigation: query direct callers, uses, or database table lineage
ripwire-rs . --callers="Interner::len"
ripwire-rs . --uses="carrier_sim_swap"   # cross-language embedded SQL table lineage
# 3. Pre-Commit Gate: name tests to run and check untested blast radius (exit 4 on failure)
ripwire-rs . --test-gate

# 4. Quality Delta: check 10 quality dimensions against git HEAD (exit 2 on regression)
ripwire-rs . --quality-delta

# 5. Agent Skills: install 17 ripwire-rs agent skills to ~/.agents/skills
ripwire-rs --add-skills

# 6. Agent Context: launch native JSON-RPC Model Context Protocol (MCP) server
ripwire-rs mcp
```

---

## Real-World Workflows

`ripwire-rs` is designed around 6 high-impact workflows. See the complete **[Workflow Guide](docs/WORKFLOWS.md)** (or online at **[ripwire-rs Workflows](https://casonadams.github.io/ripwire-rs/workflows.html)**) for detailed scenarios, commands, and benefits:

1. **[Orientation & Scoping](docs/WORKFLOWS.md#1-orientation--scoping-landing-cold-in-a-repository)**: Landing cold in an unfamiliar codebase without burning prompt token budgets (`--top-k`, `--tree`, `--for`, `--html`).
2. **[Navigation & Blast Radius](docs/WORKFLOWS.md#2-navigation--blast-radius-safe-refactoring--impact-analysis)**: Safe refactoring and dependency tracing before modifying code (`--callers`, `--callees`, `--impact`, `--around`, `--path`, `--slice`).
3. **[Pre-Commit & PR Safety Gates](docs/WORKFLOWS.md#3-pre-commit--pr-safety-gates-zero-broken-contracts)**: Catching broken API contracts and verifying test reachability before push or merge (`--edit-check`, `--affected`, `--test-gate`, `--situ`, `--quality-delta`).
4. **[Code Health & Technical Debt](docs/WORKFLOWS.md#4-code-health--technical-debt-auditing)**: Identifying dead code, high-maintenance hotspots, and hidden coupling (`--dead-code`, `--hotspots`, `--clones`, `--cochange`, `--communities`, `--external-surface`).
5. **[Deep Inspection & Agent Context](docs/WORKFLOWS.md#5-deep-inspection--agent-context-delivery)**: Precision prompting and batching for coding agents (`--expand`, `--outline`, `--pack-top-n`, `--from-trace`, `--batch`, `--json`).
6. **[Continuous Integration (CI) & PR Gates](docs/WORKFLOWS.md#6-continuous-integration-ci-workflows--automated-pr-gates)**: Automated test gating, zero quality regressions, and upstream parity checks in CI/CD pipelines (`ripwire-rs . --test-gate`, `ripwire-rs . --quality-delta`, `make validate`).

---

## Features

- **Multi-Language AST Analysis**: 36 languages indexed via Tree-Sitter (Rust, TypeScript, JavaScript, Python, Go, C, C++, CUDA, Metal, Java, C#, Swift, Bash, Ruby, PHP, Lua, Elixir, Dart, Kotlin, GDScript, JSON, TOML, YAML, Markdown, Zig, Solidity, HCL/Terraform, Scala, Erlang, HTML, CSS, SQL, Protobuf, GraphQL, Dockerfile, Objective-C/C++).
- **Language-Agnostic Embedded SQL Lineage**: Zero extra flags. Automatically extracts SQL DML (`SELECT ... FROM`, `JOIN`, `INSERT INTO`, `UPDATE`, `DELETE FROM`) out of string literals across all 36 host languages (Dart, Rust, Python, TypeScript, Go, Java, C#, etc.). Assigns typed `RefRole::Read` and `RefRole::Write` references directly to `.sql` table/view definitions in the call graph, allowing `--callers`, `--uses`, and `--impact` to seamlessly bridge database migrations, backend services, and test suites.
- **Personalized PageRank & HITS**: Transitive blast radius and structural authority calculations converging in milliseconds over lock-free sparse matrices.
- **Pre-Commit & Pre-PR Verification Gates**: Parameter arity contract verification (`--edit-check`), surgical test reachability mapping (`--affected`), test execution gate (`--test-gate`), architectural baseline sidecars (`--baseline`, `--baseline-update`), quality census (`--pin-census`), git-changed re-ranking (`--map-diff`), integration seams (`--seams`), composite PR review context (`--pr-context`), merge conflict scouting (`--merge-scout`), Delta Maintainability Model (`--dmm`), pre-hoc worktree collision forecasting (`--plan-lanes`), and code quality regressions vs git HEAD (`--quality-delta`).
- **Code Health & Architecture Enforcement**: Dead code detection (`--dead-code`), clone detection (`--clones`), Louvain community clustering (`--communities`), architectural layering rules (`--arch` with glob wildcards), Mermaid flowchart export (`--mermaid`), hierarchical zoom (`--zoom`), AST code smell linter (`--lint`, `--sarif`, `--lint-catalog`, `--lint-select`), nonlocal mutable state tracking (`--nonlocal-state`), unified 6-family quality panel (`--quality-panel`), LP64 memory layout & cache-line field affinity (`--layout`, `--field-affinity`), local reasoning (`--context-ratio`), naming consistency (`--naming-consistency`, `--naming-locals`, `--naming-calibration`), and multi-family ensemble ranking (`--ensemble`).
- **Git Churn & Coupling Analytics**: Fowler's Shotgun Surgery co-change mining and multi-file clustering (`--cochange`, `--cochange-groups`), maintenance hotspot ranking (code churn &times; cognitive complexity), bus-factor author ownership with uniform collapse (`--owners`), symbol and branch divergence tracking (`--stray-content`, `--whereis`), and historical deleted-name oracle (`--with-history`).
- **Advanced Navigation & Query DSL**: ARISE def-use dataflow slicing (`--slice`, `--slice-depth`), innermost definition resolution (`--at`), formal relationship claim verification (`--verify`), graph query DSL (`--graph-query`), composite bundles (`--pack-task`), adaptive relevance-cliff cut (`--adaptive`), executive Markdown architecture reporting (`--report`), natural-language task verb recommendation (`--help-task`), and scoped structural trigram search (`--grep`, `--grep-context`, `--and`, `--not`, `--in`).
- **Atomic AST Code Mutations**: Surgical symbol body replacement (`--replace-symbol-body`), before/after insertions (`--insert-before-symbol`, `--insert-after-symbol`), and transactional multi-file edit plans (`--edit-plan`, `--edit-payload`, `--dry-run`) with verified byte rollback.
- **Model Context Protocol (MCP), LSP Bridge & 17 Agent Skills**: Native stdio JSON-RPC MCP server exposing 33 context routing and navigation tools, stdio JSON-RPC Language Server Protocol bridge (`--lsp`), skill file security auditing (`--scan-skills`), and automated agent skill installation (`ripwire-rs --add-skills`).

---

## Documentation

- **[Workflow Guide](docs/WORKFLOWS.md)** — In-depth guide to the 6 primary workflows: when, where, and why to use each command.
- **[CLI Reference](docs/CLI.md)** — Complete command catalog, top-level flags, streaming budgets, and selector syntax.
- **[Pre-Commit Safety Gates](docs/GATES.md)** — Verification gates (`--edit-check`, `--affected`, `--quality-delta`, `--test-gate`, `--seams`, `--pr-context`, `--merge-scout`, `--arch`).
- **[Model Context Protocol (MCP)](docs/MCP.md)** — Reference for all 33 MCP tools and agent configuration.
- **[Architecture & Pipeline](docs/ARCHITECTURE.md)** — Tree-Sitter grammars, lock-free interning, CSR in-edge graph, and streaming XML.
- **[Algorithms & Academic Lenses](docs/ALGORITHMS.md)** — PageRank, HITS, Louvain modularity, clone detector, and dataflow slicing.
- **[Interactive Documentation Site](https://casonadams.github.io/ripwire-rs/)** — Showcase, command catalog, and visualizer.

---

## Installation

### Homebrew (macOS & Linux)

```bash
brew install casonadams/tap/ripwire-rs
```

### Cargo

```bash
cargo install ripwire-rs
```

### From Source

```bash
git clone https://github.com/casonadams/ripwire-rs.git
cd ripwire-rs
make install
```

Builds release binary and automatically installs manual pages (`man ripwire-rs`).

---

## MCP Server Configuration

Register `ripwire-rs` with any MCP-compatible client or coding agent:

```json
{
  "mcpServers": {
    "ripwire-rs": {
      "command": "ripwire-rs",
      "args": ["mcp"]
    }
  }
}
```

---

## Development

```bash
# Run all quality checks (fmt-check, clippy, complexity <= 15, quality delta, crap <= 30)
make all

# Run test suite under coverage
make crap

# Format code
make fmt
```
