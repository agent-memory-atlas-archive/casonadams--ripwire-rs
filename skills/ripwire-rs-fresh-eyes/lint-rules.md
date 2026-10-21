# Built-in 39-Rule Lint Catalog & Custom `--lint-rules`

Reference for `ripwire-rs-fresh-eyes` (`--lint`, `--lint-catalog`, `--lint-select`, `--lint-ignore`, and `--lint-rules=DIR`).

## 1. Built-In Catalog (`ripwire-rs --lint-catalog`)

`ripwire-rs <dir> --lint` evaluates **39 rules**. The catalog reports each rule's severity, category, applicable languages, and introduction version. Syntax rules inspect AST nodes; naming rules inspect definitions, not text inside comments or strings. Generated files and tests are not excluded.

| Family | Rules |
| :--- | :--- |
| Control flow and style | `c-style-cast`, `goto`, `do-while`, `redundant-parens`, `suspicious-semicolon`, `typedef-over-using`, `magic-number`, `empty-catch`, `self-assign`, `large-function`, `deep-nesting`, `inconsistent-return`, `unreachable-code` |
| Security | `unsafe-c-fn`, `weak-crypto` |
| Naming | `naming-short`, `naming-wordy`, `naming-series`, `naming-underscore`, `naming-case`, `naming-predicate`, `naming-setter`, `naming-confusable`, `naming-uninformative` |
| Readability | `atom-comma-operator`, `atom-embedded-crement`, `atom-assign-as-value`, `atom-nested-ternary`, `atom-implicit-predicate`, `atom-octal-literal`, `atom-reversed-subscript` |
| Performance | `cache-node-container`, `cache-vector-of-raw-ptr`, `cache-vector-of-indirect`, `cache-heap-alloc-in-loop`, `cache-pointer-chase-loop`, `cache-gather-subscript`, `cache-shared-ptr-by-value`, `cache-manual-prefetch` |

### Filtering and Formatting

```bash
ripwire-rs <dir> --lint-catalog
ripwire-rs <dir> --lint --lint-select=cache-,unsafe-
ripwire-rs <dir> --lint --lint-ignore=naming-
ripwire-rs <dir> --lint --lint-max-per-rule=20
ripwire-rs <dir> --lint --offset=20 --limit=20
ripwire-rs <dir> --lint --sarif
```

Select and ignore values are rule-name prefixes, not category names or substrings. Unknown prefixes are errors. `findings` counts selected rules; `shown`, `capped`, and the pagination attributes describe the printed window. Rule rows disclose `shown_rows` and `rows_capped`; `applicable="0"` identifies inert rules. `findings_next` appears only when rule evaluation itself was capped, never merely because the printed window was cut.

## 2. Custom Tree-Sitter Rules (`--lint-rules=DIR`)

`ripwire-rs <dir> --lint-rules=DIR` loads YAML rules from `DIR` and evaluates them alongside built-in rules:

```yaml
# rules/no-printf.yml
- id: no-printf
  language: cpp
  severity: warn
  message: use LOG() instead of printf
  query: |
    (call_expression function: (identifier) @fn (#match? @fn "^printf$")) @hit
```

- `query:` is a tree-sitter S-expression where `@hit` marks the finding location.
- Optional structural combinators refine hits by AST containment:
  - `inside:` — keep the hit only if enclosed by a node matching this query.
  - `not-inside:` — drop the hit if enclosed by a node matching this query.
  - `not-matches:` — drop the hit if a companion capture matches this query.
