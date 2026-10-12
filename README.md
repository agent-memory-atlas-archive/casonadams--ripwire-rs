# ripwire

High-throughput agent context router and deterministic static analysis engine.
`ripwire` (`rw`) analyzes source code across 31 languages to construct precise call graphs, calculate symbol impact via PageRank and HITS, mine co-change patterns, slice intra-procedural dataflow, and enforce pre-commit quality gates. It runs as a standalone CLI or as a Model Context Protocol (MCP) server over stdio for AI coding agents.
---

## Quickstart

```bash
# 1. Orientation: generate ranked architecture map (top 200 symbols)
rw . --top-k=200

# 2. Navigation: query direct callers and transitive blast radius
rw . --callers="Interner::len"

# 3. Pre-Commit Gate: name tests to run and check untested blast radius (exit 4 on failure)
rw . --test-gate

# 4. Agent Context: launch native JSON-RPC Model Context Protocol (MCP) server
rw mcp
```

---

## Real-World Workflows

`rw` is designed around 6 high-impact workflows. See the complete **[Workflow Guide](docs/WORKFLOWS.md)** (or online at **[ripwire Workflows](https://casonadams.github.io/ripwire/workflows.html)**) for detailed scenarios, commands, and benefits:

1. **[Orientation & Scoping](docs/WORKFLOWS.md#1-orientation--scoping-landing-cold-in-a-repository)**: Landing cold in an unfamiliar codebase without burning prompt token budgets (`--top-k`, `--tree`, `--for`, `--html`).
2. **[Navigation & Blast Radius](docs/WORKFLOWS.md#2-navigation--blast-radius-safe-refactoring--impact-analysis)**: Safe refactoring and dependency tracing before modifying code (`--callers`, `--callees`, `--impact`, `--around`, `--path`, `--slice`).
3. **[Pre-Commit & PR Safety Gates](docs/WORKFLOWS.md#3-pre-commit--pr-safety-gates-zero-broken-contracts)**: Catching broken API contracts and verifying test reachability before push or merge (`--edit-check`, `--affected`, `--test-gate`, `--situ`, `--quality-delta`).
4. **[Code Health & Technical Debt](docs/WORKFLOWS.md#4-code-health--technical-debt-auditing)**: Identifying dead code, high-maintenance hotspots, and hidden coupling (`--dead-code`, `--hotspots`, `--clones`, `--cochange`, `--communities`, `--external-surface`).
5. **[Deep Inspection & Agent Context](docs/WORKFLOWS.md#5-deep-inspection--agent-context-delivery)**: Precision prompting and batching for coding agents (`--expand`, `--outline`, `--pack-top-n`, `--from-trace`, `--batch`, `--json`).
6. **[Continuous Integration (CI) & PR Gates](docs/WORKFLOWS.md#6-continuous-integration-ci-workflows--automated-pr-gates)**: Automated test gating, zero quality regressions, and upstream parity checks in CI/CD pipelines (`rw . --test-gate`, `rw . --quality-delta`, `make validate`).
---

## Features
- **Multi-Language AST Analysis**: 31 languages indexed via Tree-Sitter (Rust, TypeScript, JavaScript, Python, Go, C, C++, CUDA, Metal, Java, C#, Swift, Bash, Ruby, PHP, Lua, Elixir, Dart, Kotlin, GDScript, JSON, TOML, YAML, Markdown, Zig, Solidity, HCL/Terraform, Scala, Erlang, HTML, CSS).
- **Personalized PageRank & HITS**: Transitive blast radius and structural authority calculations converging in milliseconds over lock-free sparse matrices.
- **Pre-Commit Verification Gates**: Parameter arity contract verification (`--edit-check`), surgical test reachability mapping (`--affected`), and code quality regressions vs git HEAD (`--quality-delta`, `--test-gate`).
- **Git Churn & Coupling Analytics**: Fowler's Shotgun Surgery co-change mining and maintenance hotspot ranking (code churn * cognitive complexity).
- **Intra-Procedural Dataflow Slicing**: ARISE forward and backward variable def-use slicing.
- **Model Context Protocol (MCP)**: Native stdio JSON-RPC server exposing 33 context routing and navigation tools directly to AI coding agents.

---

## Documentation

- **[Workflow Guide](docs/WORKFLOWS.md)** — In-depth guide to the 6 primary workflows: when, where, and why to use each command.
- **[Pre-Commit Safety Gates](docs/GATES.md)** — Verification gates (`--edit-check`, `--affected`, `--quality-delta`, `--test-gate`).
- **[Model Context Protocol (MCP)](docs/MCP.md)** — Reference for all 33 MCP tools and agent configuration.
- **[Architecture & Pipeline](docs/ARCHITECTURE.md)** — Tree-Sitter grammars, lock-free interning, CSR in-edge graph, and streaming XML.
- **[Algorithms & Academic Lenses](docs/ALGORITHMS.md)** — PageRank, HITS, Louvain modularity, clone detector, and dataflow slicing.
- **[Interactive Documentation Site](https://casonadams.github.io/ripwire/)** — Showcase, command catalog, and visualizer.

---

## Installation

### Homebrew (macOS & Linux)

```bash
brew install casonadams/tap/ripwire
```

### Cargo

```bash
cargo install ripwire
```

### From Source

```bash
git clone https://github.com/casonadams/ripwire.git
cd ripwire
make install
```

*Builds release binary and automatically installs manual pages (`man rw`, `man ripwire`).*

---

## MCP Server Configuration

Register `rw` with any MCP-compatible coding agent (Claude Desktop, OpenCode, Cursor, Omp):

```json
{
  "mcpServers": {
    "ripwire": {
      "command": "rw",
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
