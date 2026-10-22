# Architecture

`ripwire-rs` is a deterministic static analysis engine and agent context router implemented in pure Rust.

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

CLI and MCP adapters share transport-neutral `query::*` implementations and
canonical reports. The root-exported `Workspace` lives in `src/workspace.rs`
and loads shared SCIP-aware analysis; ranking is held in `Workspace.pr.ranks`.
MCP wraps the same XML/text in JSON-RPC `result.content[0].text`.
`Workspace.query_context(sources)` borrows the graph and ranks for the shared
`query::commands` registry; adapters do not clone them for dispatch.
`SourceReader: Sync` supplies source-query bytes through the shared registry.
`XmlWriter::finish` completes a report; `Write::flush` only flushes bytes.

Rust ingest records impl-block spans, concrete subjects, trait bounds, and
function-pointer field assignments without turning declarations or assignments
into call edges. Resolution uses actual call sites: source-spanned inline modules,
indexed module files, and lexical type identity select owners before trait
dispatch or callback invocation. Unproven or ambiguous owners decline rather
than binding to an unrelated same-named method. `--lego` uses the same Rust
trait/subject index and declaration spans for implementors and contract methods.

## Language Plugins

A language is a directory, `src/languages/<lang>/`, that holds everything specific to it:
`mod.rs` (its `LanguageSemantics` impl and helpers), `tags.scm` (the Tree-sitter query), and
optionally `grammar.txt` (a vendored C grammar: `files = parser.c scanner.c`, optional `src =`,
`include =`, `upstream = <repo> <branch> <subdir>` and `patch = <sed script>`, read by `build.rs`
and `scripts/update-grammars.sh`). Callers only see the `LanguageSemantics` trait, so shared
code never names a language. To add one, create the directory and add one line to
`register_languages!` in `src/languages/mod.rs`, which generates the `Lang` enum, the module
declaration and the semantics table. `src/languages/common/` is not a language: it holds the shared
helpers and vocabulary every plugin builds on (syntax and complexity kinds, flag facts, lint groups,
keyword fallback) and no language logic.

## Lineage & Acknowledgments

This crate is a pure Rust port and rewrite of the groundbreaking [ripwire (C++23)](https://github.com/redhat-et/ripwire) static analysis engine, originally conceived and authored by **David Brewster** at **Red Hat Emerging Technologies**.

We express our deepest gratitude to the original creators and contributors at `redhat-et/ripwire` for pioneering the concept of "the ripgrep of AI context," developing the streaming XML protocol, formulating the pre-commit quality delta heuristics, and proving the immense power of deterministic, token-budgeted code intelligence for agentic software engineering.
