# Repository instructions

## Core mission: Maximum intelligence in the smallest token footprint

- **Signal density over volume**: Every CLI and MCP output exists to give coding
  agents the highest-precision architectural, call-graph, and dataflow
  intelligence in the fewest tokens possible.
- **Present-only, zero-filler serialization**: Omit zero/unit default attributes
  (e.g., `ccx="0"`, `in="0"`, `churn="1"`, `amp="1"`), empty wrapper tags, and
  repeated per-row pagination hints (`next=`). Place pagination and window
  metadata (`shown=`, `total=`, `capped=`, `next=`) once on the enclosing
  section or root element.
- **Noise-free ranking and triage**: Filter out non-callable noise, test/fixture
  helpers (via `CorpusView::is_test_symbol`), language builtins, and prose
  backticks before ranking or counting. Partition unresolvable references into
  actionable categories (`unresolved`, `external`, `declined`) rather than
  inflating a single bucket.
- **Actionable context in one turn**: Prefer bundling the highest-value
  implementation body, resolved callee signatures (`<calls>`), architectural
  layer tags (`layer=`), and causal def-use order (`--slice`) directly into the
  response so an agent does not need follow-up round trips.

## Code structure

- Prioritize cohesion and natural readability over artificial file length
  limits. Files up to ~300-400 lines are healthy and encouraged when they keep
  related logic, data types, and tests together.
- Avoid micro-fragmentation: do not shatter straightforward logic, argument
  definitions, or tiny helpers into separate 10-50 line files or deep directory
  hierarchies. Keep directory trees shallow (target <= 4-5 levels deep).
- Prefer idiomatic Rust function signatures: use natural parameter lists or
  explicit domain config/context structs. Never pack parameters into tuples
  (`(a, b): (&str, &str)`) to circumvent argument count thresholds.
- Leverage domain and serialization traits (`CorpusView`, `LanguageSemantics`,
  `ResolutionLookup`, `XmlReport`, `GraphGaugeProvider`, `PathLayerExt`,
  `GitStampExt`, `PaginateSlice`, `JsonValueExt`) to keep calling code concise,
  uniform, and free of ad-hoc string buffering or duplicated boilerplate. Stream
  XML directly to `Write` sinks instead of allocating intermediate `String`s.
- Prefer standard in-file `#[cfg(test)] mod tests` for unit tests. Use a sibling
  `tests.rs` or integration tests under `tests/` when a test suite is genuinely
  distinct or very large. Do not create nested test directories
  (`tests/sub/mod.rs`) for unit tests.

## Lint policy

- Do not add Clippy `allow`, `expect`, command-line exclusions, or crate-level
  lint suppressions. Refactor code to satisfy the configured lints instead.
- Remove any existing Clippy suppression encountered in code being changed.
- Verify with `make clippy`

## Testing and performance

- Use `make crap` to run tests and gate on test code coverage / CRAP threshold
  (executes workspace tests under coverage and fails if any function exceeds
  CRAP 30).
- Test duration limits: Individual unit tests must complete in milliseconds. No
  test may exceed 2.5 seconds (target < 500ms). The entire workspace test suite
  must finish in under 15 seconds.
- Never perform filesystem or disk I/O in tests (no temporary files,
  directories, or disk writes). Use in-memory slices and buffers.
- Never contact external network services or third-party APIs in tests.
- Write unit tests in in-file `#[cfg(test)] mod tests` blocks or integration
  tests in `tests/`, avoiding deep nested test module folders.

## Code quality, complexity, and CRAP score

- The repository enforces strict cognitive complexity limits via `cccc`
  (SonarSource Campbell standard). Functions must maintain a cognitive
  complexity score <= 15.
- The repository enforces CRAP (Change Risk Anti-Patterns) analysis combining
  cyclomatic complexity with test code coverage. Functions with a CRAP score >
  30 are considered fragile and defect-prone.
- When modifying, extending, or fixing bugs in any function, ensure its
  cognitive complexity remains <= 15 and its CRAP score remains <= 30.
- Never introduce new functions with cognitive complexity > 15 or a CRAP
  score > 30.
- All changes must pass `make all` (which executes format check, clippy,
  cognitive complexity <= 15 via cccc, quality regression delta via ripwire, and
  tests under test code coverage with CRAP <= 30 gating).

## Adding and maintaining tree-sitter languages

- **Shared capture vocabulary**: All `queries/<lang>/tags.scm` files MUST
  strictly use the standardized capture vocabulary enforced by
  `test_all_scm_capture_vocabulary_and_patterns_valid`:
  - Definitions: `@name`,
    `@definition.{function,method,class,struct,interface,type,constant,module,macro,var,field,section}`.
    Never invent language-prefixed capture tags (e.g.
    `@definition.objc_interface`).
  - References: `@name`,
    `@reference.{call,import,extends,implementation,class,type,read,write,macro}`.
  - Imports: `@import.path` or `@reference.import`.
- **Required semantic coverage per grammar**:
  - **OOP / Trait languages** (languages with classes, structs, interfaces,
    traits, protocols, or mixins): MUST define `@reference.extends` /
    `@reference.implementation` patterns on superclass, interface, trait, and
    mixin clauses, and MUST have a fixture entry in
    `test_oop_inheritance_lego_and_uses_contract` proving both `call_lego` and
    `RefRole::Extends` resolution.
  - **Signature vs. body byte spans**: Ensure `find_def_body_start` and
    `resolve_def_body_and_end` in `src/ingest/extract.rs` accurately separate
    the declaration signature (`[sig_start_byte, sig_end_byte)`) from the body
    block (`[sig_end_byte, end_byte)`), including grammars where the body is a
    sibling node (e.g., Dart `function_signature` + sibling `function_body`).
  - **Markup / data languages** (HTML, JSON, TOML, YAML, Markdown): Index
    high-level sections (`@definition.section`) rather than tagging leaf DOM
    nodes or data values as `SymKind::Class`.
- **Selector and multi-segment names**: When a language grammar splits compound
  identifiers or selectors (such as Objective-C selector keywords), anchor `.`
  the pattern on the primary identifier to produce exactly one symbol per
  method, avoiding fragmented names.
- **Dialect compatibility**: Ensure cross-language resolution groups are updated
  in `Lang::is_compatible` (e.g., C-family: C, C++, CUDA, Metal, Objective-C;
  JVM: Java, Kotlin, Scala; Web: TypeScript, JavaScript; Erlang/Elixir;
  HTML/CSS).
- **Dependency capability**: Only mark `Lang::is_dependency_capable()` when
  syntax-backed import/include resolution is implemented, so dependency-density
  denominators remain accurate.
- **Enum ordering & serialization**: Append new entries to `enum Lang` before
  `Lang::Unknown`. Never reorder existing values because `Lang` is serialized
  across internal indices and caches.
- **Disclose blind spots**: Acknowledge dynamic dispatch (e.g. dynamic message
  sends, `performSelector:`, metatable magic) and dialect limits in tests and
  documentation.

## Documentation and manual page synchronization

- Whenever adding or updating CLI flags, options, subcommands, defaults, or MCP
  tool definitions, the repository documentation (`README.md`, `docs/CLI.md`,
  `docs/`), the static GitHub Pages showcase in `www/` (`www/index.html`,
  `www/docs.html`), AND the manual page in `man/man1/` (`man/man1/ripwire-rs.1`)
  MUST be updated and synchronized in the same commit.
- Ensure all relative links, code examples, synopsis syntax, tool names, command
  name (`ripwire-rs`), and feature lists stay fully aligned across
  repository docs, the `www/` site, and roff manual page.

## Completion

- Run `make all` and ensure all checks pass before finishing:
  1. `fmt-check` (code formatting)
  2. `clippy` (lints treated as errors)
  3. `complexity` (cognitive complexity <= 15 via `cccc`)
  4. `quality` (quality delta regressions vs git HEAD via `ripwire-rs`)
  5. `crap` (workspace tests under coverage and CRAP <= 30 threshold gating)
