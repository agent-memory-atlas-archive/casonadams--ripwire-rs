# Pre-Commit Safety Gates

`ripwire-rs` includes zero-configuration verification gates for pre-commit hooks, developer workflows, and CI pipelines.

Top-level gate verbs and the `gate` subcommand share CLI analysis and output
settings, including SCIP overlays, cache/crawl options, JSON, and legend removal.
For example: `ripwire-rs --json gate . --quality-delta` or
`ripwire-rs --legend=none --scip=index.scip gate . --edit-check=compute`.

---

## 1. Contract Checker (`--edit-check=SYM`)

Verifies whether an edited symbol's signature changed in parameters, arity, or public visibility compared to git `HEAD`. Detects and reports all 1-hop callers incompatible with the new contract.

```xml
<edit-check schema="ripwire-rs.edit-check/v1" sym="compute" t="fn" p="src/service.rs:10" status="contract-change" defs="1" callers="2" incompatible="1">
  <c n="legacy_worker" p="src/legacy_caller.rs:15" incompatible="1">
    <site line="18" args="2" required="3"/>
  </c>
  <c n="modern_worker" p="src/modern_caller.rs:20"/>
</edit-check>
```

- **Exit Code**: Non-zero if `incompatible > 0`.
- **Disclosures**: Explicitly reports argument counts and required parameter counts at each callsite.
- **Rows**: Every caller row is emitted; incompatible callers always print, and `--limit=N --offset=M` pages the compatible ones (`shown_unflagged=`, `total=`, `has_more=`, `next_offset=`).

---

## 2. Test Reachability Gate (`--affected=FILES`)

Maps modified files and symbols to transitively reachable test files across the in-process call graph.

```xml
<affected schema="ripwire-rs.affected/v1" files="src/service.rs" seeds="1" seed_test_files="0" tests="2" reached="4">
  <test p="tests/test_service.rs" partner="1" hops="2" run_unknown="1"/>
  <test p="tests/test_billing.rs" hops="3" run_unknown="1"/>
</affected>
```

- **Ranking**: Test files are prioritized by hop distance and coupling evidence.
- **Transitive Reach**: Identifies tests that execute modified symbols even when located multiple calls away.
- **Partner Tests**: A test named after a changed file is listed with `partner="1"` even without a call edge (scripts run by a test harness): `test_X`, `X_test`, `X_spec`, `X.test`, `X.spec` in any indexed language, plus `XTest` inside a `test/`/`tests/` dir. Every candidate is listed, even when several share a subject. A changed file with no symbols names no partner, and a changed test with no symbols gets no row. A changed test that is also a partner keeps its row and gets `partner="1"`. Runner-less rows that agree on every flag fold into one `<g n= p=>` row. `--test-gate` exits 4 while any test row exists.

---

## 3. Quality Delta Gate (`--quality-delta`)

Enforces regression-free commits across 10 architectural quality dimensions against git `HEAD` or a pinned baseline (`--quality-baseline`):
1. **Cognitive Complexity** ($CCX \le 15$ Campbell standard)
2. **Cyclomatic Complexity** (McCabe paths)
3. **Function Length / Line Count**
4. **AST Duplication / Clones** (Type-1, Type-2, Type-3)
5. **Nesting Depth**
6. **Parameter Count**
7. **Dead Code Detection** (unreachable internal symbols)
8. **Short-Horizon Churn Spans** (frequently touched code regions)
9. **New Public API Surface**
10. **Baseline Regression Tracking** (supports `.ripwire-rs_quality_acks` suppression ledger)

```xml
<quality-delta schema="ripwire-rs.quality-delta/v1" baseline="git-HEAD" regressions="0" minor="0" acked="0" stale="0" preexisting-worse="0" new-symbol="0" gating="0" register-macro-excluded="0" api-new-surface="0" at="9e5c9d5+dirty">
</quality-delta>
```

- **Radical Honesty**: Discloses working copy status (`+dirty`), new public API surface, and separates non-blocking new-symbol debt from blocking pre-existing regressions.
- **Dirty Pins**: `--quality-baseline` refuses on a dirty tree that already gates against `HEAD`; `--allow-dirty` pins anyway, stamps the absorbed count, and later deltas carry `baseline_absorbed="N"` (green then means clean since the pin).
- **Stale Acks**: a regression-free delta collapses stale `.ripwire-rs_quality_acks` rows to `stale_by_kind=` plus a `stale_next=` replay; `--show-stale` prints every row.
- **Exit Code**: Returns exit code 2 if blocking quality regressions exist (`gating > 0`).

---

## 4. Test Obligation Gate (`--test-gate`)

The definitive pre-commit verification gate combining changed symbols, affected test discovery, and untested blast radius analysis.

```xml
<test-gate schema="ripwire-rs.test-gate/v1" changed="1" impacted="12" tests="1" untested="1" untested_modscope="0" shown_tests="1" tests_capped="0" shown_untested="1" untested_capped="0" script_gates_unmodelled="0" ccx_bar="15" run_first="1" at="6e9b85eb8" root="." next="python3 tests/test_service.py">
  <t p="tests/test_service.py" partner="1" run="python3 tests/test_service.py"/>
  <u sym="new_helper" p="src/service.py" l="42" ccx="9"/>
</test-gate>
```

- **Exit Code**: Returns exit code 4 while any `<t>` test row or `<u>` untested impacted symbol row exists.
- **Runners**: Derives `run=` from the test file's language (`python3` when `__main__` is present, `bash` for `.sh`, `npx vitest run` / `npx jest` from the nearest `package.json`, `node --test` when `require('node:test')` is present) and emits `run_unknown="1"` otherwise.
- **Untested Blast Radius**: `<u>` rows list impacted non-test symbols that no test transitively reaches, sorted by cognitive complexity descending and paged by `--limit`/`--offset`; `<file-scope>` owners are counted separately in `untested_modscope=`.

---

## 5. Integration Seams Gate (`--seams`)

Identifies call edges crossing module or directory boundaries that have zero direct test coverage.

```xml
<seams schema="ripwire-rs.seams/v1" root="." seams="12">
  <seam caller="src/api::handle" callee="src/db::query" cross="api->db"/>
</seams>
```

- **Architecture Audit**: Pinpoints vulnerable boundary interfaces before integration testing.

---

## 6. Map Diff Gate (`--map-diff`)

Re-ranks the architecture map toward git-modified files so recent work automatically floats to the top of the context window.

```bash
ripwire-rs . --map-diff --top-k=30
```

- **Focus**: Keeps LLM context windows concentrated on currently active development tasks without losing structural context.

---

## 7. Composite PR Context (`--pr-context=BASE`)

Assembles a complete pull request review context bundle against a base git reference (defaulting to `main` or `master`).

```xml
<pr-context schema="ripwire-rs.pr-context/v1" base="main" files="2">
  <file p="src/service.rs" symbols="14">
    <changed-symbols count="1">
      <s n="execute"/>
    </changed-symbols>
    <impact dependents="5" files_other="2"/>
    <tests count="2"/>
    <cochange commits="12" partners="1"/>
    <owners authors="2" bf="1"/>
  </file>
</pr-context>
```

- **One-Turn Review**: Combines changed symbols, transitive impact, tests to run, co-change partners, and authorship in a single query.

---

## 8. Branch Merge Scout (`--merge-scout="branch-a,branch-b"`)

Predicts semantic and symbol merge conflicts across concurrent divergent branches before attempting a git merge or rebase.

```xml
<merge-scout schema="ripwire-rs.merge-scout/v1" base="main" branches="feature-a,feature-b" conflicts="1">
  <conflict sym="Interner::len" file="src/domain.rs" branches="feature-a,feature-b"/>
</merge-scout>
```

---

## 9. Architectural Layering Rules (`--arch=RULES`)

Enforces directional dependency constraints between architectural layers from a simple configuration file.

```text
# rules.txt
layer domain <= core
layer core <= api
deny api -> domain
```

```bash
ripwire-rs . --arch=rules.txt
```

- **Exit Code**: Returns exit code 2 if any forbidden cross-layer dependency edge is introduced.

---

## 10. Situational Awareness (`--situ`)

Emits canonical `ripwire-rs.situ/v1` XML summarizing a working tree diff: blast radius, tests to run, forgotten co-change partners, maintenance hotspots touched, and modules modified. The MCP `situational_awareness` tool returns the same report in `result.content[0].text`.
