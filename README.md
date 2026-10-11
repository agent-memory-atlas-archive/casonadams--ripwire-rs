# ripwire

High-throughput agent context router and deterministic static analysis engine.

`ripwire` analyzes source code across 24 languages to construct precise call
graphs, calculate symbol impact via PageRank, mine co-change patterns, slice
intra-procedural dataflow, and enforce pre-commit quality gates. It runs as a
standalone CLI (`rw`) or as a Model Context Protocol (MCP) server over stdio for
AI coding agents.

---

## Features

- **Multi-Language AST Analysis:** 24 languages indexed via Tree-Sitter (Rust,
  TypeScript, JavaScript, Python, Go, C, C++, CUDA, Metal, Java, C#, Swift,
  Bash, Ruby, PHP, Lua, Elixir, Dart, Kotlin, GDScript, JSON, TOML, YAML,
  Markdown).
- **Graph & Impact Analysis:** Directed call graphs, 1-hop caller/callee
  neighborhoods, and transitive blast radius calculation ranked by PageRank.
- **Pre-Commit Verification Gates:**
  - Contract arity and visibility breakage check (`--edit-check`).
  - Transitive test reachability mapping (`--affected`).
  - Codebase quality delta regression checking against git HEAD
    (`--quality-delta`).
- **Git Analytics:** Fowler's Shotgun Surgery co-change coupling and maintenance
  hotspot ranking (code churn \* cognitive complexity).
- **Intra-Procedural Slicing:** ARISE forward and backward variable def-use
  dataflow slicing.
- **Model Context Protocol (MCP):** Native JSON-RPC stdio server exposing
  context tools directly to AI coding agents.

---

## Getting Started

### Prerequisites

- Rust 1.80+ (2021 edition)
- C compiler (`clang` or `gcc`) for compiling Tree-Sitter grammars
- Git

### Build

Grammar sources are fetched on demand to keep the repository lightweight:

```bash
# Fetch upstream Tree-Sitter grammars
make update-grammars

# Build release binary
make build-release
```

Binary will be located at `target/release/rw`.

---

## Usage

### CLI Overview

```bash
# General repository overview and call graph map
rw .

# Show maintenance hotspots (churn * cognitive complexity)
rw --hotspots

# Fowler's Shotgun Surgery co-change coupling
rw --cochange [FILE]

# Check caller compatibility after editing a symbol
rw --edit-check "symbol_name"

# Map affected test files for changed symbols or files
rw --affected "src/lib.rs"

# Measure quality delta regressions against git HEAD
rw --quality-delta

# ARISE variable dataflow slicing (fwd, back, both)
rw --slice "function_name:var_name" --slice-flow fwd
```

### Subcommands

```bash
# Exploration and task-scoped orientation
rw orient [PATH]

# Callers, callees, and transitive impact
rw navigate [PATH] --symbol <NAME> --impact

# Pre-commit verification gates
rw gate [PATH] --edit-check <SYM> --affected <PATH> --quality-delta

# Complexity and maintenance hotspots
rw metrics [PATH] --hotspots

# Launch stdio Model Context Protocol (MCP) server
rw mcp
```

---

## MCP Server Configuration

To wire `ripwire` into an MCP client (such as Claude Desktop, OpenCode, or Omp),
register the executable:

```json
{
  "mcpServers": {
    "ripwire": {
      "command": "/path/to/rw",
      "args": ["mcp"]
    }
  }
}
```

---

## Development

```bash
# Run all quality checks (fmt, clippy, complexity, quality delta, CRAP/coverage)
make all

# Run test suite under coverage
make crap

# Format code
make fmt
```
