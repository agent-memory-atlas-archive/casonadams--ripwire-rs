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

# Export self-contained interactive SVG/HTML visualizer
rw . --html=callgraph.html --color-by=community
```

## Pre-Commit Verification Gates

```bash
# Check caller compatibility after editing a symbol contract
rw . --edit-check="symbol_name"

# Transitive reachability mapping to affected test suites
rw . --affected="src/lib.rs"

# Snapshot code quality baseline at current commit
rw . --quality-baseline

# Check for quality regressions against baseline or git HEAD
rw . --quality-delta
```

## Code Intelligence & Metrics

```bash
# Detect AST code clones and near-miss duplicates (Type-1, Type-2, Type-3)
rw . --clones --min-tokens=40

# Mine historical git co-change coupling (Fowler's Shotgun Surgery)
rw . --cochange

# Compute maintenance hotspots (churn * cognitive complexity)
rw . --hotspots

# Compute ARISE variable def-use dataflow slice (forward/backward)
rw . --slice="compute:x" --slice-flow=fwd
```

## Workflow Subcommands

- `rw orient [PATH]`: Codebase orientation and structural summaries.
- `rw navigate [PATH] --symbol <SYM> [--callers|--callees|--impact]`: Symbol neighborhood analysis.
- `rw gate [PATH] [--edit-check|--affected|--quality-delta]`: Verification safety gates.
- `rw metrics [PATH] [--hotspots|--complexity]`: Code health metrics.
- `rw slice [PATH] --symbol <SYM> --var <VAR>`: Intra-procedural slicing.
- `rw mcp`: Launch native JSON-RPC Model Context Protocol (MCP) server over stdio.
