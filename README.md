# ripwire-rs

High-throughput agent context router and deterministic static analysis engine.

Rust port and rewrite of [redhat-et/ripwire](https://github.com/redhat-et/ripwire), originally created and authored by David Brewster at Red Hat Emerging Technologies.

`ripwire-rs` analyzes source code across 39 languages to construct precise call graphs, calculate symbol impact via PageRank and HITS, mine co-change patterns, slice intra-procedural dataflow, and enforce pre-commit quality gates. It runs as a standalone CLI or as a Model Context Protocol (MCP) server over stdio for AI coding agents.


> **100% Local & Zero Telemetry**: `ripwire-rs` is an offline-by-design, security-first static analysis engine. It contains **zero network calls, zero telemetry, zero analytics, and zero model training**. Your source code never leaves your machine. It purely extracts local AST context, call graphs, and metrics, piping structured XML/JSON to stdout or stdio MCP for whichever agent harness or workflow engine *you* choose to run. Automated secret redaction is enabled by default.
---

## Design Principles

Built for **maximum intelligence in the smallest token footprint**:

- **Present-Only Serialization**: Omits zero/unit default attributes (`ccx="0"`, `in="0"`, `churn="1"`) and places pagination window metadata (`shown=`, `total=`, `capped=`, `next=`) once on the root element.
- **Noise-Free Ranking**: Filters out non-callable noise, test/fixture helpers, language builtins, and prose backticks before ranking; partitions unresolvable calls into `unresolved`, `external`, and `declined`.
- **Actionable Context in One Turn**: Bundles top implementation bodies, resolved callee signatures (`<calls>`), architectural layer tags (`layer=`), and causal def-use slices (`--slice`) directly into responses.
- **Sub-50ms Zero-Buffer Pipeline**: Interned `Spur`/`NodeId` tables, contiguous CSR graphs, in-process `gix` history walking, and direct `io::Write` XML/JSON streaming across 39 Tree-Sitter languages (with automatic embedded SQL table lineage).
- **100% Local & Zero Telemetry**: Offline-by-design. Zero network calls, zero tracking, zero model training, and zero data leakage. Code never leaves your workstation; `ripwire-rs` purely extracts AST context and feeds it to your chosen local or remote agent harness. Secret redaction is enabled by default.

---

## Installation

```bash
# macOS & Linux (Homebrew)
brew install casonadams/tap/ripwire-rs

# Ubuntu & Debian (APT)
echo "deb [trusted=yes] https://casonadams.github.io/homebrew-tap/apt stable main" | sudo tee /etc/apt/sources.list.d/casonadams.list
sudo apt update && sudo apt install ripwire-rs

# Fedora, RHEL & CentOS (DNF / RPM)
sudo dnf config-manager --add-repo https://casonadams.github.io/homebrew-tap/rpm/casonadams.repo
sudo dnf install ripwire-rs

# Rust (prebuilt binary via cargo-binstall, or compile from source)
cargo binstall ripwire-rs
cargo install ripwire-rs
```

See the **[Installation Guide](docs/INSTALL.md)** for standalone `.deb`/`.rpm` downloads, source builds (`make install`), and post-install setup.

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

# 7. Multi-Agent Fleet Orchestration: partition task bundles, plan parallel worktrees, and scout branch merges
ripwire-rs . --pack-task="refactor serialization" --partition=4
ripwire-rs . --plan-lanes=3 --task="refactor serialization"
ripwire-rs . --merge-scout="branch-a,branch-b"
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
7. **[Multi-Agent Fleet Orchestration](docs/WORKFLOWS.md#7-multi-agent-fleet-orchestration-parallel-worktrees--subagent-slicing)**: Community-carved task partitioning, pre-hoc worktree lane collision planning, cross-branch merge scouting, and session handoffs (`--pack-task --partition=N`, `--plan-lanes`, `--brief`, `--merge-scout`, `--handoff`, `--help-task`, `--verify`).

Navigation reports expose their limits: `--impact` lists unbound candidate call
sites as `<dc>` rows, never as graph edges. `--uses` lists all indexed sites by
default; add `--limit=N --offset=M` to page them. Literal search reports span
tiers and suppressed mentions; `--grep-in=any` includes comments and strings,
and `--in=PATH` restricts paths. `--whereis` scans distinct Git blobs up to 2 MiB
once, including unindexed HEAD files and changed worktree paths.
Python, JavaScript/TypeScript, and Rust also index bare non-local variable reads;
grep's `reads=` comes from those references, not text estimates. Unsupported
binding forms remain evidence floors.

---

## Documentation

- **[Installation Guide](docs/INSTALL.md)** — Homebrew, APT, DNF/RPM, `cargo-binstall`, and source build instructions.
- **[Workflow Guide](docs/WORKFLOWS.md)** — In-depth guide to the 7 primary workflows: when, where, and why to use each command.
- **[AGENTS.md Governance Guide](docs/AGENTS_GUIDE.md)** — Drop-in `AGENTS.md` template, 5-step autonomous verification loop, and CI/CD parity.
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

## Supported Languages (39)

First-class Tree-Sitter AST queries extract definitions, signatures, imports, and call sites across 39 languages, with automatic embedded SQL table lineage (`SELECT`, `JOIN`, `INSERT`, `UPDATE`, `DELETE`) resolved across host-language string literals.

| Domain | Supported Languages & Extensions |
| :--- | :--- |
| **Systems & Native** | **Rust** (`.rs`), **C** (`.c`, `.h`), **C++** (`.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx`, `.hh`, `.h`), **Zig** (`.zig`, `.zon`), **Go** (`.go`), **Objective-C / Objective-C++** (`.m`, `.mm`), **Fortran** (`.f`, `.for`, `.f90`, `.f95`, `.f03`, `.f08`), **COBOL** (`.cbl`, `.cob`, `.cpy`) |
| **GPU & Shaders** | **CUDA** (`.cu`, `.cuh`), **Metal** (`.metal`) |
| **Web & Frontend** | **TypeScript** (`.ts`, `.tsx`, `.mts`, `.cts`, `.astro`), **JavaScript** (`.js`, `.jsx`, `.mjs`, `.cjs`), **HTML** (`.html`, `.htm`), **CSS / Sass / Less** (`.css`, `.scss`, `.sass`, `.less`) |
| **JVM & Enterprise** | **Java** (`.java`), **Kotlin** (`.kt`, `.kts`), **Scala** (`.scala`, `.sc`), **C#** (`.cs`) |
| **Scripting & Backend** | **Python** (`.py`, `.pyi`), **Ruby** (`.rb`, `.rake`, `.jbuilder`), **PHP** (`.php`, `.phtml`), **Elixir** (`.ex`, `.exs`), **Erlang** (`.erl`, `.hrl`), **Lua** (`.lua`), **Bash / Shell** (`.sh`, `.bash`, `.zsh`) |
| **Mobile, Game & Media** | **Swift** (`.swift`), **Dart** (`.dart`), **GDScript** (`.gd`), **BrightScript** (`.brs`, `.bs`) |
| **Data, Schema & Smart Contracts** | **SQL** (`.sql`), **GraphQL** (`.graphql`, `.gql`), **Protocol Buffers** (`.proto`), **Solidity** (`.sol`) |
| **Infra, Config & Docs** | **HCL / Terraform** (`.tf`, `.hcl`, `.tfvars`), **Dockerfile** (`Dockerfile`, `.dockerfile`, `.containerfile`), **YAML** (`.yml`, `.yaml`), **TOML** (`.toml`), **JSON** (`.json`), **Markdown / Docs** (`.md`, `.markdown`, `.mdx`, `.rst`, `.adoc`, `.org`) |

---

## Development

```bash
# Run all quality checks (clippy, complexity <= 15, quality delta, crap <= 30, fmt)
make all

# Run test suite under coverage
make crap

# Format code
make fmt
```
