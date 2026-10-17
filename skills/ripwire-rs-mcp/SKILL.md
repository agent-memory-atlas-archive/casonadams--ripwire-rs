---
name: ripwire-rs-mcp
description: >
  Configure ripwire-rs as a stdio Model Context Protocol (MCP) server and diagnose MCP tool
  health with `ripwire-rs --doctor`.
allowed-tools: Bash, Read
---

<identity>
You are an MCP (Model Context Protocol) integration specialist. Your goal is to wire ripwire-rs into coding agents, diagnose tool health, and select optimal server verbs.
</identity>

<investigate_before_configuring>
Before configuring MCP:
1. Audit active server diagnostics: `ripwire-rs <dir> --doctor`
2. Verify available tools: check tools/list via stdio JSON-RPC.
</investigate_before_configuring>

<instructions>
# ripwire as an MCP server

> Auditing somebody's `.mcp.json` for safety instead → **ripwire-rs-security-scan**.

Trigger: "wire ripwire into my agent", "set up the ripwire MCP server", or "which ripwire MCP verb
answers this?".

**The 35 MCP tools, at a glance** (`33` primary tools + `callers`/`callees` aliases, backed by `McpToolHandler`):
- **17 read verbs**: `analyze`, `rank_by`, `for`, `find_symbol`, `find_referencing_symbols` (plus `callers` and `callees` aliases), `grep`, `cochange`, `memory_recall`, `situational_awareness`, `mentions`, `owners`, `lego`, `batch` (up to 16 read sub-queries in one call), `fetch_body`, `flags`, `doc_drift`, and `slice`.
- **13 flagship-reflex verbs**: `exemplar`, `quality_delta`, `quality_baseline`, `impact`, `uses`, `affected`, `path_between`, `connect`, `explore`, `from_trace`, `edit_check`, `whereis`, and `stray_content`.
- **3 atomic span-addressed edit verbs**: `replace_symbol_body`, `insert_before_symbol`, `insert_after_symbol`.

## Wiring — Starting the stdio MCP server

```json
{
  "mcpServers": {
    "ripwire-rs": {
      "command": "ripwire-rs",
      "args": ["mcp"]
    }
  }
}
```

Run `ripwire-rs <repo> --doctor` to verify binary PATH agreement, grammar compilation, cache health, and git reachability.
### Workspace Root & Caching

Start the server with `ripwire-rs <repo> --mcp` or `ripwire-rs mcp` (defaulting to `.`). The server maintains an in-memory `McpState` backed by `.ripwire.cache` so subsequent tool calls answer in milliseconds without re-parsing unchanged files.

## The read verbs + fetch_body (and when each beats the CLI form)

Every verb takes `path` (the repo root; `memory_recall` takes the docs/memory dir). For a split
service+client checkout, pass `paths: [dir1, dir2, ...]` instead — the additive multi-root array (`path` and
`paths` together is a usage error; the 3 edit verbs accept it too, and land a write in the correct root's
real file via its label). The server keeps ONE cached workspace per canonical root set: a `paths` list is
deduped and ordered before it's used as the cache key, so calling with the same roots in a different order
still hits the warm cache instead of re-parsing; a genuinely different root set gets its own cache entry, not
a shared/stale one.

**Line-seeded addressing — `@FILE:LINE` in any resolver-backed selector.** Holding a LOCATION (a diff hunk,
a compiler error, a stack frame) instead of a name? Pass `@src/foo.cpp:120` (1-based line) as the selector
and it resolves to the innermost definition enclosing that line — accepted by `find_symbol` /
`find_referencing_symbols` / `impact` / `edit_check` / `path_between` (`from`/`to`) / `connect` (each entry) /
`lego` (`type`) / `fetch_body` (`handle`), and by `uses` (which serves the enclosing definition's NAME —
its sites stay name-matched, `of=` echoes the seed as typed). A bad seed — malformed spec, unmatched or
ambiguous path, line past EOF, a line no definition spans, two definitions sharing the line — is **refused
with the same specific diagnosis the CLI's `--at`/selector arms speak**, never guessed. The NAME-matching
scan verbs (`owners`, `mentions`) do not resolve seeds; a resolvable seed there refuses by naming the
definition it resolves to, so the retry is in the message. CLI twin: `--at=FILE:LINE` (the bare
enclosing-chain report) and `@FILE:LINE` in any SYM selector; contract gate: `test/atcheck.sh` +
`test/mcpverbscheck.sh` §7.

| Verb | CLI twin | Ask it for |
|---|---|---|
| `analyze` | `ripwire-rs <dir>` | the ranked XML map |
| `for` (`task`) | `--for=TASK` | the task lens: signatures + cx/in metrics framed for reuse. **Auto-routes** the ranker — pass a symbol NAME verbatim as `task` to get name-exact retrieval (recall@1 ~99%); a conceptual phrase uses subtoken+body. Root carries `route=` as a code (`name-exact(X)` / `subtoken+body[:broad\|:declined]`). |
| `find_symbol` (`symbol`) | `--callers` + `--callees` | locate a symbol with its callers AND callees in one call — each symbol carries a `handle` |
| `find_referencing_symbols` (`symbol`) | `--callers=SYM` | just who references/calls it — also handle-bearing |
| `grep` (`pattern`) | `--grep=STR` | parallel literal scan + enclosing symbol + matched line |
| `cochange` (`file`) | `--cochange=FILE` | the lockstep git partners of one file |
| `memory_recall` (`task`, `top_k` + `budget_tokens` optional) | `--recall=TASK [--top-k=N] [--max-tokens=N]` | full bodies of the few relevant docs/memory notes, bounded by the SAME default 8000-token body ceiling as the CLI (the header discloses `max_tokens=` and every cut). `budget_tokens` raises the ceiling explicitly when you want everything; `top_k` (default 8) shapes how many docs |
| `situational_awareness` (`diff`/`files` optional) | `--situ` | blast radius, tests_to_run, forgotten co-change partners (the Shotgun Surgery check), hotspot alert — as JSON; defaults to `git diff HEAD`. In `tests_to_run`, `situational_awareness` uses `test`; `explore` and edit receipts use `p`. The field is a path string OR an **array** of paths beside `n` — several runner-less tests sharing their attributes, served as one row — and every row carries `run` or `run_unknown: true` |
| `mentions` (`symbol`) | `--mentions=SYM` | which markdown plans/designs discuss a symbol |
| `owners` (`symbol` optional) | `--owners[=SYM]` | bus-factor: recency-weighted author ownership |
| `lego` (`type`) | `--lego=TYPE` | an interface's method contract + every implementor (own-language) |
| `fetch_body` (`handle`) | `--expand=SYM` | the FULL source of one symbol's definition, addressed by a `handle` from a prior read verb |
| `batch` (`queries`) | `--batch=FILE` | a one-turn context sweep: up to 16 heterogeneous read sub-queries (`for`/`grep`/`impact`/`uses`/`callers`/`callees`/`mentions`/…) answered in ONE round-trip, merged in order and deduped (`<dup-of q="i"/>`); a failing sub-query is an inline `ok="0"` entry, never a whole-batch failure. Reach for it when a task needs several lookups at once. |

**The flagship-reflex verbs** (added 2026-07 so an MCP-only agent gets the same reflexes as the CLI) — these are the moments an agent most often skips the tool for; reach for the verb, not a grep:

| Verb | CLI twin | The moment |
|---|---|---|
| `exemplar` (`kind` or `task`) | `--exemplar` | BEFORE you write a fn/class/… — the repo's best-in-class instance to imitate, chosen by ROLE (fan-in / cognitive-cx / tested), with its body |
| `quality_delta` (`path`) | `--quality-delta` | BEFORE you call it DONE — only what the working tree made WORSE vs the baseline (auto-compares vs git-HEAD; a non-empty `regressions` array is the exit-2-equivalent) |
| `quality_baseline` (`path`) | `--quality-baseline` | pin the quality floor (writes the HEAD-stamped `.ripwire_quality_baseline`) — a side-effect verb |
| `impact` (`symbol`, `limit`/`offset` optional) | `--impact=SYM [--limit=N --offset=M]` | "is it safe to change X" — the TRANSITIVE blast radius (beats `find_referencing_symbols`, which is 1-hop). `reaches=` is the true radius; the listing shows 40 by rank unless you raise it, and a paged answer carries `has_more`/`next_offset` so a loop can terminate |
| `uses` (`symbol`) | `--uses=SYM` | the resolvable USE-SITES — call/read/write/import/extends, not just calls (`count=0` is a real answer; `counts_floor="1"` — a floor, never a total) |
| `path_between` (`from`, `to`) | `--path=A,B` | does A reach B, and the shortest call path (named `path_between` — `path` is the root-arg key) |
| `connect` (`symbols`) | `--connect=A,B,C` | the minimal subgraph joining N (>2) task symbols — the shared-caller join a directed `path_between` can't see |
| `explore` (`task`, `budget_tokens` + `partition` optional) | `--pack-task=TASK [--partition=N]` | ONE-call task orientation — routed ranking + full bodies + 1-hop callers + field notes + tests_to_run, ALL under one deterministic byte budget (default 6000 tokens). Replaces the `for` → `fetch_body` → `find_referencing_symbols` → `memory_recall` dance. Also dispatchable as `pack_task` (same handler; `explore` is the one advertised in `tools/list` — the discovery-friendly name). **`partition: 2..16`** turns it into the FAN-OUT form for a multi-agent orchestrator: a shared common core plus N minimally-overlapping per-agent slices carved along the call graph's communities, so N sub-agents stop re-deriving one map — `budget_tokens` then means ONE agent's budget (core + its slice). Read `overlap_max` / `split` / `partitions` vs `requested` on the wrapper before trusting the slices |
| `from_trace` (`trace`, `budget_tokens` optional) | `--from-trace=FILE` | paste a stack trace / sanitizer report / compiler error → the frames mapped onto indexed symbols, ranked INNERMOST-first, with the innermost in-corpus symbol's FULL body. `trace` = the raw trace TEXT (no stdin/file arg over MCP — paste it, don't hand-translate it into a query) |
| `edit_check` (`symbol`) | `--edit-check=SYM` | just edited a symbol? did its CONTRACT (params/publicness) change vs git HEAD, and which 1-hop callers are now PROVABLY incompatible? Fast targeted check — for the same question over a whole diff use `quality_delta` instead |

**MCP beats the CLI when you're mid-task and will ask more than once**: the server keeps the parsed
`{ingest, graph, pr, interner}` in memory, so after the first parse every verb answers warm (~ms, no shell
round-trip). **Use the CLI for report/visualization verbs not exposed over MCP** (`--report`, `--quality-panel`, `--ensemble`, `--context-ratio`, `--comment-coherence`, `--dmm`, `--hotspots`, `--clones`, `--dead-code`, `--communities`, `--zoom`, `--mermaid`, `--html`, `--deps`, `--arch`, `--seams`, `--pr-context`, `--merge-scout`, `--test-gate`, `--safe-delete`, `--exercises`, `--layout`, `--field-affinity`, `--lint`, `--notes`).
## Editor transport: `--lsp` (navigation LSP server) — read-only, Phase 1

`--mcp` answers agents and `--listen` serves a team; `--lsp` answers **editors**: a read-only,
navigation-focused LSP 3.x server over stdio — lifecycle (`initialize`/`shutdown`/`exit`) plus
`definition`, `references`, `documentSymbol` (member variables merged into the outline), workspace
symbol, and `hover`. Same warm index as `--mcp` — no second parser, no second process. Saved-state
answers only: an unsaved buffer is absent from the index, not mispositioned, and every count is a
floor, not a total (the hover text says so in place, since JSON-RPC results have no metadata channel).
UTF-8 positions; refuses to combine with `--mcp`/`--listen` — one protocol per stdin. The PoC plan and
its locked decisions live in `docs/LSP.md`.

## The lazy-body posture: names/signatures by default, bodies by handle on request

`find_symbol` and `find_referencing_symbols` attach a stable `handle` to every symbol object they return
(the focus symbol, each caller, each callee) — but **not the body**. Call `fetch_body{path, handle}` only
when you actually need to read the source, instead of the server sending bodies you didn't ask for. This is
the kit-style default-lean posture (measured ~90% cut on comparable extract-symbol calls) and matches the
`2025-11-25` MCP spec's move to stateless HANDLE semantics.

- **Handle format**: `sym#<idHash>@<contentHash>`, both 16 lowercase hex (FNV-1a-64) — `idHash` from the
  symbol's STABLE canonical id (`path::scope::name`, never the per-run `NodeId`), `contentHash` pinning the
  file's bytes at mint time.
- **Fetch when you're about to reason about the logic**, not just to confirm the symbol exists — the
  name/kind/file/line a read verb already gives you is often enough to decide relevance or pick a call site.
- **Staleness is automatic and free**: a content-hash mismatch (file changed since mint) makes `fetch_body`
  refuse rather than serve a body against shifted byte offsets — call a read verb again for a fresh handle.
  A malformed/unresolvable handle refuses the same way, never a silent wrong-symbol body.
- **Handles are content-addressed, not pinned literals**: both halves are derived hashes, so a handle you
  copied out of a prior session or a doc is almost certainly stale — call `find_symbol`/
  `find_referencing_symbols` fresh each session and read the `handle` it hands back.

## The per-session legend: each definition once per session

On the stdio server the legend session opens with the first answer that carries a legend (served inline) or
with a read of the MCP resource `ripwire://legend-dict` (`initialize` names it). After that, the verbs that take
`legend` (and `for`) answer rows first and end with `<about … legend="ref" dict= dictv=/>`, which carries the
root attributes; a definition arrives once per session, in a comment after the rows of the first answer that
needs it (an answer whose ref form would be longer stays inline). Lost the definitions (a compacted context)?
Read `ripwire://legend-dict/full` for all of them, or print them with `ripwire-rs --legend-dict`.
`legend:"compact"` on one call keeps that answer's legend inline; a server started with
`--mcp --mcp-legend=inline` keeps every answer's legend inline. The HTTP transport (`--listen`) keeps every
legend inline.

## The 3 MCP edit verbs — the warm-server counterpart to the preferred CLI

`replace_symbol_body`, `insert_before_symbol`, `insert_after_symbol` — the last 3 of the 30 (see above),
ripwire's MCP WRITE verbs. The preferred shell front door is
`ripwire-rs ROOT --replace-symbol-body=SYM --edit-payload=FILE|-` (or the matching insert flag); use MCP when
the server is already warm or a client has no shell. Both fronts call the same safety engine. Each locates
a symbol's definition in the parsed index and splices text at its byte span: `replace_symbol_body`
swaps signature-through-closing-brace verbatim, `insert_before_symbol`/`insert_after_symbol` splice text at
the def's first/final byte (auto-adding the separating newline so you don't have to guess).

**The receipt already answers "what now?"** — it carries `lines={start,end}` for the applied text,
`edit_check` (status / callers / incompatible / each broken caller's call lines) and `tests_to_run` with the
run recipe: the same answers a separate `edit_check` verb and `--affected` would give, computed on the index
the edit just refreshed. Do not spend two more calls on them. `post_check:false` (CLI `--no-post-check`)
opts out — reach for it only when the next thing you do is another edit to the same tree.

Read **[`mcp-reference.md`](mcp-reference.md)** before calling one of these 3 (full args table + the exact
newline rule + the safety contract for not-found / ambiguous / stale-index / insane-span failures + the
atomic-write guarantee) or when you need server internals (`--mcp` implies `--stable`, `_index` staleness
stamps, warm content-hash rebuilds, working-set PageRank personalization, the background `qsnap` HEAD-warm on
large workspaces). Short version: every edit-verb failure leaves the file byte-for-byte unchanged, writes are
atomic (`tmp` + `rename(2)`), and the server never goes stale silently — it rebuilds warm from mtime checks
before every verb call. **Lead with this MCP form when the body already came from `fetch_body` this
session**; after a CLI `--expand`, keep the zero-standing-schema CLI path and use `--edit-payload=-`.
Several harnesses' native Edit tool
needs a fresh Read of the file immediately before the edit, even when you already have the body from
elsewhere in the session — these verbs check their own staleness hash instead of requiring one, so under any
harness the file gets read at most once, never twice.

**Workspace pin (stdio, D3/D4):** when the server was started `ripwire-rs <root> mcp`, an omitted
`path` on ANY verb (read or edit) defaults to that startup root; the 3 edit verbs above additionally
REFUSE (file byte-identical, same contract as every other refusal above) if an explicit `path` resolves
outside it — `path outside workspace; start the server on that root or pass an absolute in-root path`.
Read verbs stay unrestricted at any `path`. A bare `ripwire-rs mcp` with no startup root is unchanged: every
verb still requires its own `path`.

<example title="Configuring ripwire-rs MCP server">
Good: Register `ripwire-rs` with `"command": "ripwire-rs", "args": ["mcp"]` in your MCP client config and verify with `ripwire-rs <dir> --doctor`.
Bad: Manually hardcoding fragile temporary paths or unsupported flags.
</example>
</instructions>

<output_format>
### MCP Configuration
- **Transport Mode**: [Stdio JSON-RPC 2.0]
- **Configuration Stanza**: [Exact JSON / TOML / CLI snippet]
- **Health Status**: [Diagnostic verification result]
</output_format>
