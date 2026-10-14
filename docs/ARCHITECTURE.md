# Architecture

`ripwire` is a deterministic static analysis engine and agent context router implemented in pure Rust.

## Pipeline Overview

```
[Candidate Source Files]
           │
           ▼
┌──────────────────────┐
│ Deterministic Crawl  │  Byte-sorted path ordering, denylist pruning, size bounds
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ Parallel AST Ingest  │  36 Tree-Sitter grammars, lock-free lasso interning, SoA tables
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ Reference Resolution │  Lexical precedence ladder: same-file > same-dir > global
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ CSR In-Edge Graph    │  Transpose Compressed Sparse Row matrix for cache-line locality
└──────────┬───────────┘
           │
           ├────────────────────────┬────────────────────────┐
           ▼                        ▼                        ▼
┌──────────────────────┐ ┌──────────────────────┐ ┌──────────────────────┐
│ PageRank / HITS      │ │ Community Detection  │ │ Academic Lenses      │
│ 1024-block reduction │ │ Louvain modularity   │ │ ARISE, Clones, Churn │
└──────────┬───────────┘ └──────────┬───────────┘ └──────────┬───────────┘
           │                        │                        │
           └────────────────────────┴────────────────────────┘
                                    │
                                    ▼
┌──────────────────────┐
│ Streamed XML Output  │  64 KB BufWriter direct-to-stdout without document accumulation
└──────────────────────┘
```

## Key Architectural Invariants

1. **Deterministic Execution**:
   - File crawling collects all valid source paths and sorts them lexicographically by byte order before assigning IDs.
   - All symbol IDs, graph traversals, and algorithms use deterministic tie-breaking (no random seeds, no hash iteration order leakage).

2. **Memory Efficiency**:
   - Symbols and references reside in Structure-of-Arrays (SoA) vectors addressed by 32-bit `NodeId` handles.
   - String interning backed by concurrent `lasso::ThreadedRodeo` eliminates duplicate string allocations.
   - Graph representations utilize Compressed Sparse Row (CSR) matrices for high compute density and cache locality.

3. **Streaming XML Serialization**:
   - Output serializes directly to a 64 KB `BufWriter` on `stdout`.
   - Never accumulates the full serialized document in memory.
   - Enforces configurable pack budgets (default 64 KB / 16K tokens) to prevent context saturation.

4. **Language-Agnostic Embedded SQL Lineage**:
   - Zero extra flags: executes automatically during standard parallel AST ingest.
   - String literals, raw strings, and template strings across all host languages (Dart, Rust, Python, TypeScript, Go, Java, C#, etc.) are scanned for SQL DML keyword sequences (`SELECT ... FROM`, `JOIN`, `INSERT INTO`, `UPDATE`, `DELETE FROM`).
   - Keyword pairs classify table accesses into typed reference roles (`RefRole::Read` and `RefRole::Write`).
   - The reference resolution ladder binds these table accesses directly to `.sql` DDL definitions (`CREATE TABLE ...`), unifying database entities into the same CSR in-edge graph as application functions and methods.
## Lineage & Acknowledgments

This crate is a pure Rust port and rewrite of the groundbreaking [ripwire (C++23)](https://github.com/redhat-et/ripwire) static analysis engine, originally conceived and authored by **David Brewster** at **Red Hat Emerging Technologies**.

We express our deepest gratitude to the original creators and contributors at `redhat-et/ripwire` for pioneering the concept of "the ripgrep of AI context," developing the streaming XML protocol, formulating the pre-commit quality delta heuristics, and proving the immense power of deterministic, token-budgeted code intelligence for agentic software engineering.
