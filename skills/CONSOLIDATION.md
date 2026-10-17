# Skills consolidation — 30 → 17 (Wave 3, 2026-07)

The skills audit found 30 user-facing skills collapsing into
~8 jobs with mutually indistinguishable descriptions — a routing hazard and a token tax. This pass merged
them into **17 routable skills**, each answerable to "when would an agent pick THIS one over every other".

No content was invented; every merged skill preserves the best material of its sources (the concrete
use-when cues + honesty calibration of the 2025-refreshed skills), and every command line runs against this
repo. Absorbed skill directories were deleted — their content lives on in the merged skill named below.

## Old skill → new home

| Old skill            | New home                     | Notes |
|----------------------|------------------------------|-------|
| ripwire-rs-orient       | **ripwire-rs-orient**           | kept; now the escalation-ladder base |
| ripwire-rs-tour         | **ripwire-rs-orient**           | folded in as the report→deps→communities→hotspots ladder |
| ripwire-rs-zoom         | **ripwire-rs-orient**           | folded in as the nested-hierarchy + --mermaid/--html rungs |
| ripwire-rs-navigate     | **ripwire-rs-navigate**         | kept; --query-vs---for + graph-query pointer retained |
| ripwire-rs-explain      | **ripwire-rs-navigate**         | folded in as the one-symbol deep-dive section |
| ripwire-rs-triage       | **ripwire-rs-find-bug**         | → Branch A ("symptom, no idea where") |
| ripwire-rs-bisect       | **ripwire-rs-find-bug**         | → Branch B ("suspect a subsystem, narrow it") |
| ripwire-rs-diagnose     | **ripwire-rs-find-bug**         | → Branch C ("I changed X and it broke" — --situ) |
| ripwire-rs-design       | **ripwire-rs-before-you-build** | → feasibility spike / reusable blocks |
| ripwire-rs-plan         | **ripwire-rs-before-you-build** | → plan section |
| ripwire-rs-spike        | **ripwire-rs-before-you-build** | → feasibility-spike section |
| ripwire-rs-scope        | **ripwire-rs-before-you-build** | → sizing-rubric section |
| ripwire-rs-interface    | **ripwire-rs-before-you-build** | → interface section (design-time boundary work) |
| ripwire-rs-pr-impact    | **ripwire-rs-change-check**     | keeps --map-diff + --rank-by=churn |
| ripwire-rs-pre-pr       | **ripwire-rs-change-check**     | keeps --map-diff + --rank-by=churn |
| ripwire-rs-fresh-eyes   | **ripwire-rs-fresh-eyes**       | kept; now the repo-wide once-over base |
| ripwire-rs-clone-hunt   | **ripwire-rs-fresh-eyes**       | folded into duplicate-bodies pass |
| ripwire-rs-owners       | **ripwire-rs-fresh-eyes**       | folded into ownership / bus-factor pass |
| ripwire-rs-dead-code    | **ripwire-rs-fresh-eyes**       | folded in; dead-code's honest caveats preserved verbatim |
| ripwire-rs-coupling     | **ripwire-rs-fresh-eyes**       | folded into the hidden-coupling (co-change) pass |
| ripwire-rs-audit-skill  | **ripwire-rs-security-scan**    | → skill-file scanner section |
| ripwire-rs-mcp-audit    | **ripwire-rs-security-scan**    | → .mcp.json manual-checklist section |
| ripwire-rs-efficient    | **ripwire-rs-efficient**        | kept standalone |
| ripwire-rs-quality-bar  | **ripwire-rs-quality-bar**      | kept; cross-ref updated pre-pr → change-check |
| ripwire-rs-reuse-first  | **ripwire-rs-reuse-first**      | kept standalone |
| ripwire-rs-review       | **ripwire-rs-change-check** + **ripwire-rs-fresh-eyes** | **DELETED** — split by moment (see 2026-07 MOMENT pass below) |
| ripwire-rs-compress     | **ripwire-rs-compress**         | kept standalone (the detail-ladder rewrite); later merged into ripwire-rs-efficient, see 2026-07-05 below |
| ripwire-rs-handoff      | **ripwire-rs-handoff**          | kept standalone |
| ripwire-rs-layers       | **ripwire-rs-layers**           | kept standalone (--arch gate + baseline workflow) |
| ripwire-rs-mcp          | **ripwire-rs-mcp**              | kept; cross-ref updated mcp-audit → security-scan |
| ripwire-rs-graph-query  | **ripwire-rs-graph-query**      | kept standalone |
| ripwire-rs-perf-target  | **ripwire-rs-perf-target**      | kept standalone; description sharpened + routing header added |
| —                    | **ripwire-rs-write-tests**      | NEW (S4, 2026-07-10) — no predecessor; untested-code coverage moment had no home until then |

## Judgment calls beyond the target taxonomy

- **interface → before-you-build** and **coupling → fresh-eyes** (the audit flagged both as optional). Both
  moves make descriptions MORE distinguishable: interface is a design-*time* boundary activity alongside
  spike/plan/scope; coupling is a hidden-dependency *sweep* over the same descriptive facts fresh-eyes
  already surfaces. This left `interface`/`coupling` from having near-duplicate "assess structure" triggers.
- **layers kept standalone** rather than folded into fresh-eyes: it owns the CI-enforceable `--arch=rules.txt`
  gate + baseline workflow (referenced elsewhere, e.g. from orient) — a distinct "is my architecture drifting"
  moment vs fresh-eyes' "what's the state of this code" once-over.
- **perf-target kept standalone** (it was the 30th skill, unplaced by the target taxonomy): "where should I
  optimize before profiling" is a moment no other skill claims. Description sharpened + routing header added.

Net: 17 skills (7 merged homes + 10 kept), each with a discriminating description and a routing header naming
its nearest neighbours.

## MOMENT pass (2026-07) — route by moment, not by feature

The 17 were well-written individually but agents **route by MOMENT**, and three moments were co-claimed by
near-synonym descriptions (router stalls → agent falls back to grep). This pass rewrote DESCRIPTIONS to be
pickable from the description alone, split `ripwire-rs-review` by moment, and added an entry-point router.

- **WRITE moment de-collided.** `ripwire-rs-reuse-first` = "about to write ONE symbol (fn/class/helper)";
  `ripwire-rs-before-you-build` = "about to start a FEATURE (multi-symbol, needs a plan/interface/sizing)";
  `ripwire-rs-efficient` reframed as a pure cross-cutting DISCIPLINE ("token+accuracy discipline for any read"),
  dropping "about to author" so it stops competing at the write moment. `--exemplar` now surfaced in
  before-you-build too (was reachable only from reuse-first).
- **`ripwire-rs-review` DELETED, split by moment:** its diff-review material (blast radius, tests, hotspots,
  `--metrics` interpretation) → **ripwire-rs-change-check**; its unfamiliar-subsystem-risk + refactor-planning
  material → **ripwire-rs-fresh-eyes** (now scope-to-a-subsystem, not repo-only). change-check + quality-bar are
  now an explicit CHAIN ("did the code get worse" → "is it safe to merge").
- **UNDERSTAND de-duped:** the verbatim "how does X work / where is Y" now lives only on `ripwire-rs-orient`;
  `ripwire-rs-efficient` dropped it.
- **Stranded features got interpretation homes:** `--metrics` attrs (`cbo`/`lcom4`/`nest`/`loc`/`params`/
  `tested`) interpreted with thresholds+actions in change-check; refactor/god-object trigger + `--communities`/
  `--cochange` in fresh-eyes; portable `--cache=FILE` one-liner in efficient.
- **Router added:** `ripwire-rs-router` = the moment→skill map (10+ moments → one skill each) + the two-reflex
  primer (before you write → `--exemplar`; before done → `--quality-delta`). Every skill also gained a
  one-line routing header so a wrong route self-corrects in one hop.

Net: 17 skills (review deleted, router added). The count is a rounding error; the win is every recognized
moment has exactly one clear entry.

## 2026-07-05 — ripwire-rs-compress merged into ripwire-rs-efficient (17 → 16)

`ripwire-rs-compress`'s detail ladder (map → `--pack-signatures` → `--outline` → `--expand` → `--pack-top-n`)
and its `--compress` scope/when-NOT-to-compress guidance never had its own MOMENT — agents already land in
`ripwire-rs-efficient` first (the map-before-you-read discipline), and `--compress` is the next question once
you're already reading a body `--expand`/`--outline` surfaced. Moved verbatim (plus the redaction/
`--no-redact` note, A3-S5) into `skills/ripwire-rs-orient/compress-ladder.md`, a companion file loaded on
demand — same pattern as `skills/ripwire-rs-quality-bar/quality-metrics.md`. `ripwire-rs-efficient/SKILL.md` gained one
pointer line; every `ripwire-rs-compress` reference elsewhere (router's reference-skills list) was repointed.
Directory `skills/ripwire-rs-compress/` deleted.

Net: 16 skills (compress folded into efficient as a companion file, no moment lost).

## 2026-07-10 — ripwire-rs-write-tests added (S4) (16 → 17)

The untested-code-coverage moment ("this is untested, add coverage") had no home — it's distinct from
judging your own diff (`ripwire-rs-change-check`) or your own code's quality (`ripwire-rs-quality-bar`). New
standalone skill `ripwire-rs-write-tests`: ranks candidates by `--seams` (untested cross-module edges) and the
`tested=1` coverage lens, pulls the symbol's outside contract via `--callers`, verifies the new test registers with
`--affected`. No skill was merged or deleted for this one — it's a genuinely new moment.

Net: 17 skills (write-tests added, no other change).

## 2026-09-07 — the efficient skill folded into ripwire-rs-orient (18 → 17 dirs; 16 routable + router)

The description-budget round (issue #49; `docs/EVALS.md` "Skill descriptions under a client budget") put
every description in front of three blind LLM raters — full text, head-cut at 350, and the rewrite. The one
loss that survived in every model and every arm was the efficient skill's rows routing to `ripwire-rs-orient`:
raters could not see the boundary even from the full 977-character description, because "about to open
several files to answer one question" is the same moment as "understand this fast". The fold arm had zero
such misses. So the directory is gone: its `SKILL.md` became `skills/ripwire-rs-orient/map-before-you-read.md`
(the discipline, verbatim), `compress-ladder.md` moved beside it, orient's description claims the moment, the
router's four rows point at orient, the corpus's 12 rows carry the mechanical relabel, and `--help-task`'s
`compact-legend` intent names orient.

Net: 17 skill directories (16 routable + the router; `ripwire-rs-opt-remarks` stays contributor-gated at install).
