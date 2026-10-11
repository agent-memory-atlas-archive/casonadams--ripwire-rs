# Repository instructions

## Code structure

- Prioritize cohesion and natural readability over artificial file length limits.
  Files up to ~300-400 lines are healthy and encouraged when they keep related
  logic, data types, and tests together.
- Avoid micro-fragmentation: do not shatter straightforward logic, argument
  definitions, or tiny helpers into separate 10-50 line files or deep directory
  hierarchies. Keep directory trees shallow (target <= 4-5 levels deep).
- Prefer idiomatic Rust function signatures: use natural parameter lists or
  explicit domain config/context structs. Never pack parameters into tuples
  (`(a, b): (&str, &str)`) to circumvent argument count thresholds.
- Prefer standard in-file `#[cfg(test)] mod tests` for unit tests. Use a sibling
  `tests.rs` or integration tests under `tests/` when a test suite is genuinely distinct
  or very large. Do not create nested test directories (`tests/sub/mod.rs`) for unit tests.

## Lint policy

- Do not add Clippy `allow`, `expect`, command-line exclusions, or crate-level
  lint suppressions. Refactor code to satisfy the configured lints instead.
- Remove any existing Clippy suppression encountered in code being changed.
- Verify with `make clippy`

## Testing and performance

- Use `make crap` to run tests and gate on test code coverage / CRAP threshold
  (executes workspace tests under coverage and fails if any function exceeds CRAP 30).
- Test duration limits: Individual unit tests must complete in milliseconds.
  No test may exceed 2.5 seconds (target < 500ms). The entire workspace
  test suite must finish in under 15 seconds.
- Never perform filesystem or disk I/O in tests (no temporary files, directories, or disk writes). Use in-memory slices and buffers.
- Never contact external network services or third-party APIs in tests.
- Write unit tests in in-file `#[cfg(test)] mod tests` blocks or integration tests
  in `tests/`, avoiding deep nested test module folders.

## Code quality, complexity, and CRAP score

- The repository enforces strict cognitive complexity limits via `cccc` (SonarSource Campbell standard). Functions must maintain a cognitive complexity score <= 15.
- The repository enforces CRAP (Change Risk Anti-Patterns) analysis combining cyclomatic complexity with test code coverage. Functions with a CRAP score > 30 are considered fragile and defect-prone.
- When modifying, extending, or fixing bugs in any function, ensure its cognitive complexity remains <= 15 and its CRAP score remains <= 30.
- Never introduce new functions with cognitive complexity > 15 or a CRAP score > 30.
- All changes must pass `make all` (which executes format check, clippy, cognitive complexity <= 15 via cccc, quality regression delta via ripwire, and tests under test code coverage with CRAP <= 30 gating).

## Completion

- Run `make all` and ensure all checks pass before finishing:
  1. `fmt-check` (code formatting)
  2. `clippy` (lints treated as errors)
  3. `complexity` (cognitive complexity <= 15 via `cccc`)
  4. `quality` (quality delta regressions vs git HEAD via `ripwire`)
  5. `crap` (workspace tests under coverage and CRAP <= 30 threshold gating)
