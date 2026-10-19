# ripwire-rs

High-throughput agent context router and deterministic static analysis engine.

Rust port and rewrite of [redhat-et/ripwire](https://github.com/redhat-et/ripwire), originally created and authored by David Brewster at Red Hat Emerging Technologies.

`ripwire-rs` analyzes source code across 36 languages to construct precise call graphs, calculate symbol impact via PageRank and HITS, mine co-change patterns, slice intra-procedural dataflow, and enforce pre-commit quality gates. It runs as a standalone CLI or as a Model Context Protocol (MCP) server over stdio for AI coding agents.

---

## Design Principles

Built for **maximum intelligence in the smallest token footprint**:

- **Present-Only Serialization**: Omits zero/unit default attributes (`ccx="0"`, `in="0"`, `churn="1"`) and places pagination window metadata (`shown=`, `total=`, `capped=`, `next=`) once on the root element.
- **Noise-Free Ranking**: Filters out non-callable noise, test/fixture helpers, language builtins, and prose backticks before ranking; partitions unresolvable calls into `unresolved`, `external`, and `declined`.
- **Actionable Context in One Turn**: Bundles top implementation bodies, resolved callee signatures (`<calls>`), architectural layer tags (`layer=`), and causal def-use slices (`--slice`) directly into responses.
- **Sub-50ms Zero-Buffer Pipeline**: Interned `Spur`/`NodeId` tables, contiguous CSR graphs, in-process `gix` history walking, and direct `io::Write` XML/JSON streaming across 36 Tree-Sitter languages (with automatic embedded SQL table lineage).

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

## Quickstart

```bash
# 1. Orientation & Scoping: ranked architecture map or task-focused bundle
ripwire-rs . --top-k=200
ripwire-rs . --for="implement user authentication"

# 2. Navigation & Blast Radius: callers, transitive impact, or def-use slice
ripwire-rs . --callers="Interner::len"
ripwire-rs . --impact="Interner"

# 3. Pre-Commit & PR Safety Gates: affected tests and contract verification
ripwire-rs . --test-gate
ripwire-rs . --edit-check="Interner::len"

# 4. Code Health & Architecture: quality regressions, hotspots, and clones
ripwire-rs . --quality-delta
ripwire-rs . --hotspots

# 5. Deep Inspection & Agent Context: compressed bodies or stack-trace mapping
ripwire-rs . --expand="Interner::len" --compress

# 6. Agent Skills & MCP Server: install 17 skills or run stdio MCP server
ripwire-rs --add-skills
ripwire-rs mcp
```

---

## Workflows

See the **[Workflow Guide](docs/WORKFLOWS.md)** (or **[online](https://casonadams.github.io/ripwire-rs/workflows.html)**) for complete scenarios:

1. **[Orientation & Scoping](docs/WORKFLOWS.md#1-orientation--scoping-landing-cold-in-a-repository)**: Cold-start repository maps and task-ranked context (`--top-k`, `--tree`, `--for`, `--html`).
2. **[Navigation & Blast Radius](docs/WORKFLOWS.md#2-navigation--blast-radius-safe-refactoring--impact-analysis)**: Dependency tracing, SQL table lineage, and dataflow slicing (`--callers`, `--callees`, `--impact`, `--uses`, `--connect`, `--slice`).
3. **[Pre-Commit & PR Safety Gates](docs/WORKFLOWS.md#3-pre-commit--pr-safety-gates-zero-broken-contracts)**: Contract checks, test reachability, and diff awareness (`--edit-check`, `--affected`, `--test-gate`, `--situ`, `--pr-context`).
4. **[Code Health & Technical Debt](docs/WORKFLOWS.md#4-code-health--technical-debt-auditing)**: Quality regressions, dead code, hotspots, clones, and coupling (`--quality-delta`, `--quality-panel`, `--dead-code`, `--hotspots`, `--clones`, `--cochange`, `--deps`).
5. **[Deep Inspection & Agent Context](docs/WORKFLOWS.md#5-deep-inspection--agent-context-delivery)**: Token-budgeted symbol bodies, trace mapping, and multi-query batching (`--expand`, `--outline`, `--compress`, `--from-trace`, `--batch`, `--json`).
6. **[CI Gates & Agent Integration](docs/WORKFLOWS.md#6-continuous-integration-ci-workflows--automated-pr-gates)**: Automated PR gates, 33-tool stdio MCP server, LSP bridge, and 17 agent skills (`--add-skills`, `ripwire-rs mcp`, `--lsp`, `--scan-skills`).

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
