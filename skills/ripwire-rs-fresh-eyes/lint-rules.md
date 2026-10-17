# Built-in 43-Rule Lint Catalog & Custom `--lint-rules`

Reference for `ripwire-rs-fresh-eyes` (`--lint`, `--lint-catalog`, `--lint-select`, `--lint-ignore`, and `--lint-rules=DIR`).

## 1. Built-In 43-Rule Catalog (`ripwire-rs --lint-catalog`)

`ripwire-rs <dir> --lint` evaluates **43 trait-backed rules** (`10 SymbolLintRule` + `33 FileLintRule`):

| Category | Rules | Scope & What They Catch |
| :--- | :--- | :--- |
| **`maintainability` (7)** | `complexity-spike`, `deep-nesting`, `parameter-spill`, `verbosity-spike`, `dead-function`, `large-function`, `unreachable-code` | Functions exceeding cognitive complexity (`>15`), nesting (`>5`), parameter count (`>6`), length (`>100` / `>200` LOC), zero-caller private functions, and statements after unconditional `return`/`break`/`continue`/`throw`/`goto`. |
| **`naming` (8)** | `naming-short`, `naming-wordy`, `naming-underscore`, `naming-case`, `naming-series`, `naming-confusable`, `naming-uninformative`, `naming-predicate`, `naming-setter` | Single-char or overly wordy (`>5` subtokens) names, double/edge underscores, mixed camel+snake case, numbered series (`foo1`/`foo2`), Levenshtein-1 confusable pairs, generic `data`/`info`/`tmp` names, non-boolean `is_*`/`has_*` predicates, and value-returning `set_*` methods. |
| **`confusion` (7)** | `atom-implicit-predicate`, `atom-nested-ternary`, `atom-assign-as-value`, `atom-comma-operator`, `atom-embedded-crement`, `atom-octal-literal`, `atom-reversed-subscript` | Gopstein FSE 2017 atoms of confusion: nested `? :` ternaries, assignment inside `if`/`while` conditions, comma expressions, `++`/`--` embedded inside larger expressions, leading-zero octal literals (`0755`), and `idx[arr]` reversed subscripts. |
| **`style` (8)** | `goto`, `do-while`, `c-style-cast`, `typedef-over-using`, `magic-number`, `redundant-parens`, `suspicious-semicolon`, `inconsistent-return` | `goto` and `do-while` loops, C-style `(type)expr` casts in C/C++, C++ `typedef` over `using`, magic numeric literals, `if ((x))` redundant parens, `if (...);` stray semicolons, and mixed bare `return;` vs `return val;`. |
| **`safety` (5)** | `unsafe-c-fn`, `weak-crypto`, `empty-catch`, `self-assign`, `shared-ptr-by-value` | Unbounded C functions (`strcpy`, `strcat`, `sprintf`, `vsprintf`, `gets`), MD5/SHA-1/DES weak crypto, empty `catch` / `except: pass` blocks, `x = x` self-assignment, and `std::shared_ptr<T>` passed by value (atomic refcount churn). |
| **`performance` (7)** | `cache-node-container`, `cache-vector-of-raw-ptr`, `cache-virtual-in-loop`, `cache-boolean-field-cluster`, `cache-heap-alloc-in-loop`, `cache-pointer-chase-loop`, `cache-gather-subscript`, `cache-manual-prefetch` | Pointer-chasing containers (`std::list`, `std::map`, `std::set`), `std::vector<T*>` indirect buffers, virtual dispatch or heap allocations inside loops, `>=3` adjacent `bool` fields (use bitfields/flags), `p = p->next` linked-list loops, `a[b[i]]` gather indexing, and manual `__builtin_prefetch` / `_mm_prefetch`. |

### Filtering and Formatting

```bash
ripwire-rs <dir> --lint-catalog                             # list all 43 rules with severity and category
ripwire-rs <dir> --lint --lint-select=cache-,safety         # run only matching rule prefixes
ripwire-rs <dir> --lint --lint-ignore=naming-               # exclude matching rule prefixes
ripwire-rs <dir> --lint --lint-max-per-rule=20              # cap findings per rule
ripwire-rs <dir> --lint --sarif                             # emit SARIF 2.1.0 JSON for GitHub Code Scanning
```

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
