# Pre-Commit Safety Gates

`ripwire` includes zero-configuration verification gates for pre-commit hooks and CI pipelines.

## 1. Contract Checker (`--edit-check=SYM`)

Verifies whether an edited symbol's signature changed in parameters, arity, or public visibility compared to git `HEAD`. Detects and reports all 1-hop callers incompatible with the new contract.

```xml
<edit-check schema="ripwire.edit-check/v1" sym="compute" p="src/service.rs" status="contract-change" callers="2" incompatible="1">
  <c n="legacy_worker" p="src/legacy_caller.rs:15" incompatible="1">
    <site line="18" args="2" required="3"/>
  </c>
  <c n="modern_worker" p="src/modern_caller.rs:20"/>
</edit-check>
```

- **Exit Code**: Non-zero if `incompatible > 0`.
- **Disclosures**: Explicitly reports argument counts and required parameter counts at each callsite.

---

## 2. Test Reachability Gate (`--affected=FILES`)

Maps modified files and symbols to transitively reachable test files across the in-process call graph.

```xml
<affected schema="ripwire.affected/v1" files="src/service.rs" seeds="1" seed_test_files="0" tests="1" reached="4">
  <test p="tests/test_service.rs" hops="2"/>
</affected>
```

- **Ranking**: Test files are prioritized by hop distance and coupling evidence.
- **Transitive Reach**: Identifies tests that execute modified symbols even when located multiple calls away.

---

## 3. Quality Delta Gate (`--quality-delta`)

Enforces regression-free commits across 11 architectural quality dimensions:
1. Cyclomatic Complexity ($CCX \le 15$)
2. Function Length / Verbosity
3. Dead Code Detection
4. AST Duplication / Clones
5. Nesting Depth
6. Parameter Count

```xml
<quality-delta schema="ripwire.quality-delta/v1" baseline="git-HEAD" regressions="0" minor="0" acked="0" stale="0" preexisting-worse="0" new-symbol="0" gating="0" register-macro-excluded="0" api-new-surface="0" at="9e5c9d5+dirty">
</quality-delta>
```

- **Radical Honesty**: Discloses working copy status (`+dirty`), new public API surface, and separates non-blocking new-symbol debt from blocking pre-existing regressions.
- **Exit Code**: Returns exit code 1 if `gating > 0`.
