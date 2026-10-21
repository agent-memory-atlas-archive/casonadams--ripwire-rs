# Repository instructions

## Core mission: Maximum intelligence in the smallest token footprint

`ripwire-rs` exists to replace blind `grep` and whole-file reads with deterministic, token-budgeted codebase intelligence across 39 languages, delivering the exact symbols, blast radius, and safety verification an agent or developer needs in one turn.

- **Seven core workflows**: Every feature serves one of (1) *Orientation & Scoping* (`--top-k`, `--for`, `--tree`), (2) *Navigation & Blast Radius* (`--callers`, `--impact`, `--uses`, `--connect`, `--slice`, plus zero-flag embedded SQL table lineage), (3) *Pre-Commit & PR Safety Gates* (`--edit-check`, `--affected`, `--test-gate`, `--situ`), (4) *Code Health & Architecture* (`--quality-delta`, `--quality-panel`, `--hotspots`, `--clones`, `--cochange`, `--deps`, `--dmm`), (5) *Deep Inspection & Context Delivery* (`--expand`, `--outline`, `--compress`, `--from-trace`, `--batch`, `--verify`), (6) *Agent & CI Integration* (`ripwire-rs mcp`, `--lsp`, `--add-skills`, `--scan-skills`, `--help-task`), or (7) *Multi-Agent Fleet Orchestration* (`--pack-task --partition=N`, `--plan-lanes=N --task=GOAL`, `--plan-lanes --brief=FILE`, `--merge-scout`, `--handoff`, `--notes`, `--note-add`).
- **Present-only serialization**: Omit zero/unit default attributes (e.g.,
  `ccx="0"`, `in="0"`, `churn="1"`, `amp="1"`), empty wrapper tags, and repeated
  per-row pagination hints. Place window metadata (`shown=`, `total=`,
  `capped=`, `next=`) once on the enclosing element.
- **Noise-free ranking**: Filter out non-callable noise, test/fixture helpers,
  language builtins, and prose backticks before ranking or counting. Partition
  unresolvable references into `unresolved`, `external`, and `declined`.
- **Actionable context in one turn**: Bundle the highest-value implementation
  body, resolved callee signatures (`<calls>`), architectural layer tags
  (`layer=`), and causal def-use order (`--slice`) directly into responses.

## Performance and code structure

- **Latency targets**: `< 50ms` warm on medium repos (~10k symbols), `< 500ms` on
  100k+ symbol monorepos, and strictly faster than upstream C++ `ripwire`
  (`make parity`, `make validate-knowledge`, `make bench`).
- **Deterministic, zero-buffer pipeline**: Sort crawled paths lexicographically
  and use deterministic tie-breaking everywhere (never leak hash iteration order).
  Keep hot-path work on interned `Spur` / `NodeId` integers and contiguous CSR
  graphs. Stream XML and JSON directly to `io::Write` sinks (`XmlReport`,
  `XmlEscapeExt`) rather than building intermediate `String`s in loops. Walk git
  history in-process via `gix`; never spawn `git` subprocesses in loops.
- **Idiomatic signatures**: Use natural parameter lists or domain context
  structs; never pack parameters into tuples (`(a, b): (&str, &str)`) to bypass
  argument limits.

## Tree-sitter languages

- **Capture vocabulary**: `src/queries/<lang>/tags.scm` must only use the
  standardized `@name`, `@definition.*`, `@reference.*`, and `@import.path`
  captures (never language-prefixed capture tags). Anchor `.` on the primary
  identifier for multi-segment selectors so each method emits a single symbol.
- **Semantics & enum stability**: Implement language rules via
  `LanguageSemantics` in `src/languages/`. Append new `enum Lang` variants
  before `Lang::Unknown`; never reorder existing variants (serialized in cache).

## Testing, linting, and verification

- **No lint suppressions**: Never add Clippy `allow` / `expect` attributes or
  CLI exclusions; remove any encountered in touched code and refactor instead.
- **Fast, in-memory public-API tests**: Place all tests in `tests/<mod>/<file>.rs`
  (rooted by `tests/<mod>.rs`), never inline `#[cfg(test)]` in `src/`. Never
  perform disk I/O or network calls in tests; use in-memory slices (`< 500ms`
  per test, `< 15s` full suite).
- **Generic names only**: Findings from other codebases must reach tests,
  fixtures, docs, and commit messages under generic names (`lint_rules.py`,
  `user_accounts`); never copy their repo, path, symbol, or table names.
- **Documentation sync**: When changing CLI flags, subcommands, defaults, or MCP
  tools, update `README.md`, `docs/`, `www/`, `man/man1/ripwire-rs.1`, and
  `skills/` in the same commit, and verify with `ripwire-rs . --doc-drift`.
- **Required gate**: Run `make all` before finishing (enforces `clippy`,
  cognitive complexity `<= 15`, `--quality-delta` vs `git HEAD`, test coverage
  CRAP `<= 30`, and `fmt` as the final step).
