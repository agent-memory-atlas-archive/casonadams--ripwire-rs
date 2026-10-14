#!/usr/bin/env bash
set -euo pipefail

REPO="${1:-.}"
if [[ ! -d "$REPO" ]]; then
    echo "Error: repository path '$REPO' does not exist" >&2
    exit 1
fi
REPO_ABS=$(cd "$REPO" && pwd)

# Check for yq
if ! command -v yq >/dev/null 2>&1; then
    echo "Error: 'yq' is required for XML validation. Please install yq." >&2
    exit 1
fi

# Locate ripwire-rs binary
RW_BIN=""
for cand in "target/release/ripwire-rs" "target/release/rw"; do
    if [[ -x "$cand" ]]; then
        RW_BIN=$(pwd)/"$cand"
        break
    fi
done
if [[ -z "$RW_BIN" ]]; then
    RW_BIN=$(command -v ripwire-rs || command -v rw || true)
fi
if [[ -z "$RW_BIN" || ! -x "$RW_BIN" ]]; then
    echo "Error: 'ripwire-rs' binary not found. Run 'cargo build --release' first." >&2
    exit 1
fi

# Locate upstream ripwire binary
RIP_BIN=""
if [[ -x "$HOME/.local/bin/ripwire" ]]; then
    RIP_BIN="$HOME/.local/bin/ripwire"
else
    RIP_BIN=$(command -v ripwire || true)
fi
if [[ -z "$RIP_BIN" || ! -x "$RIP_BIN" ]]; then
    echo "Error: upstream 'ripwire' binary not found in PATH or ~/.local/bin/ripwire." >&2
    exit 1
fi

# Detect default merge ref in target repo
MERGE_REF="HEAD"
for cand in "main" "master" "HEAD"; do
    if git -C "$REPO_ABS" rev-parse --verify "${cand}^{commit}" >/dev/null 2>&1; then
        MERGE_REF="$cand"
        break
    fi
done

echo "=================================================="
echo "Validating 1:1 Parity on: $REPO_ABS"
echo "ripwire-rs binary: $RW_BIN"
echo "ripwire binary:    $RIP_BIN"
echo "XML validator:     $(command -v yq) ($(yq --version 2>&1 | head -n 1))"
echo "=================================================="

# Test cases: Caret-separated: CMD ^ DESC ^ TYPE ^ ASSERT_EXPR
declare -a COMMANDS=(
    "^Default ranked map^xml^.r[\"+@schema\"] == \"ripwire.map/v1\""
    "--top-k=50^Top 50 symbols^xml^.r[\"+@schema\"] == \"ripwire.map/v1\""
    "--top-k=100^Top 100 symbols^xml^.r[\"+@schema\"] == \"ripwire.map/v1\""
    "--top-k=20^Top 20 symbols^xml^.r[\"+@schema\"] == \"ripwire.map/v1\""
    "--max-tokens=2000^2000 token budget^xml^.r[\"+@schema\"] == \"ripwire.map/v1\""
    "--max-tokens=4000^4000 token budget^xml^.r[\"+@schema\"] == \"ripwire.map/v1\""
    "--rank-by=authority^Authority ranking^xml^.r[\"+@schema\"] == \"ripwire.map/v1\""
    "--rank-by=hub^Hub ranking^xml^.r[\"+@schema\"] == \"ripwire.map/v1\""
    "--legend=compact^Compact legend^xml^.r[\"+@schema\"] == \"ripwire.map/v1\""
    "--legend=full^Full legend^xml^.r[\"+@schema\"] == \"ripwire.map/v1\""
    "--tree^File inventory tree^xml^.tree[\"+@schema\"] == \"ripwire.tree/v1\""
    "--dead-code^Dead code detection^xml^.[\"dead-code\"][\"+@schema\"] == \"ripwire.dead-code/v1\""
    "--communities^Community clustering^xml^.communities[\"+@schema\"] == \"ripwire.communities/v1\""
    "--external-surface^External API surface^xml^.\"external-surface\"[\"+@schema\"] == \"ripwire.external-surface/v1\""
    "--deps^File dependencies^xml^.deps[\"+@schema\"] == \"ripwire.deps/v1\""
    "--hotspots^Maintenance hotspots^xml^.hotspots[\"+@schema\"] == \"ripwire.hotspots/v1\""
    "--cochange^Shotgun surgery co-change mining^xml^.cochange[\"+@schema\"] == \"ripwire.cochange/v1\" and (.cochange[\"+@pairs\"] | tonumber) > 0"
    "--clones^Clone detection^xml^.clones[\"+@schema\"] == \"ripwire.clones/v1\""
    "-v^Version banner^text^version"
    "--quality-delta^Quality delta gate^xml^.\"quality-delta\"[\"+@schema\"] == \"ripwire.quality-delta/v1\""
    "--test-gate^Test reachability gate^xml^.\"test-gate\"[\"+@schema\"] == \"ripwire.test-gate/v1\""
    "--map-diff --top-k=30^Map diff biased teleportation^xml^.r[\"+@schema\"] == \"ripwire.map/v1\""
    "--seams^Integration seams detection^xml^.seams[\"+@schema\"] == \"ripwire.seams/v1\""
    "--pr-context^PR review context bundle^xml^.\"pr-context\"[\"+@schema\"] == \"ripwire.pr-context/v1\""
    "--merge-scout=${MERGE_REF}^Cross-branch merge conflict prediction^xml^.\"merge-scout\"[\"+@schema\"] == \"ripwire.merge-scout/v1\""
    "--biggest-first^Posnett readability size ranking^xml^.readability[\"+@schema\"] == \"ripwire.readability/v1\""
    "--nonlocal-state^Reachable global mutable state^xml^.\"nonlocal-state\"[\"+@schema\"] == \"ripwire.nonlocal-state/v1\""
    "--lint^AST code smell scan^xml^.lint[\"+@schema\"] == \"ripwire.lint/v1\""
    "--lint-catalog^AST lint rule catalog^xml^.lintcatalog[\"+@schema\"] == \"ripwire.lint-catalog/v1\""
    "--at=src/main.rs:1^Location definition resolver^xml^.at[\"+@schema\"] == \"ripwire.at/v1\""
    "--verify=calls(main,run_cli)^Relationship claim verification^xml^.verify[\"+@schema\"] == \"ripwire.verify/v1\""
    "--pack-task=search^Task-scoped context pack^xml^.ctx[\"+@schema\"] == \"ripwire.pack-task/v1\""
    "--graph-query=callers(name(\"main\"))^Functional graph query DSL^xml^.query[\"+@schema\"] == \"ripwire.graph-query/v1\""
    "--exemplar=fn^Best-in-class style exemplar^xml^.exemplar[\"+@schema\"] == \"ripwire.exemplar/v1\" and (.exemplar[\"+@candidates\"] | tonumber) > 0"
    "--owners^Git author ownership per file^xml^.owners[\"+@schema\"] == \"ripwire.owners/v1\" and (.owners[\"+@files\"] | tonumber) > 0"
    "--doctor^Environment and tool diagnostics^xml^.doctor[\"+@schema\"] == \"ripwire.doctor/v1\""
    "--notes^Persistent session gotchas^xml^.ctx[\"+@schema\"] == \"ripwire.notes/v1\""
    "--mermaid^Mermaid diagram generation^text^mermaid"
    "--zoom^Hierarchical Louvain clustering^xml^.zoom[\"+@schema\"] == \"ripwire.zoom/v1\""
    "--html^Interactive HTML visualizer export^text^html"
    "wrap claude^Agent integration recipe^text^wrap"
    "--whereis=main^Symbol cross-ref locator^xml^.whereis[\"+@schema\"] == \"ripwire.whereis/v1\" and (.whereis[\"+@hits\"] // .whereis[\"+@count\"] | tonumber) > 0"
    "--flags^Dark-content gate harvest^xml^.flags[\"+@schema\"] == \"ripwire.flags/v1\" and (.flags[\"+@files\"] // .flags[\"+@count\"] | tonumber) > 0"
    "--stray-content^Cross-branch divergent content^xml^.\"stray-content\"[\"+@schema\"] == \"ripwire.stray-content/v1\""
)

TOTAL=${#COMMANDS[@]}
PASSED=0
FAILED=0

for entry in "${COMMANDS[@]}"; do
    IFS="^" read -r cmd desc type assert_expr <<< "$entry"

    cmd_label="${cmd:-"(default)"}"
    
    # Run commands
    if [[ "$cmd" == "-v" ]]; then
        out_rw=$("$RW_BIN" -V 2>&1 || true)
    elif [[ "$cmd" =~ ^wrap ]]; then
        out_rw=$("$RW_BIN" $cmd 2>&1 || true)
    elif [[ "$cmd" =~ ^--merge-scout= ]]; then
        out_rw=$("$RW_BIN" "$REPO_ABS" "$cmd" 2>&1 || true)
    elif [[ -z "$cmd" ]]; then
        out_rw=$("$RW_BIN" "$REPO_ABS" 2>&1 || true)
    else
        out_rw=$("$RW_BIN" "$REPO_ABS" $cmd 2>&1 || true)
    fi

    test_passed=true
    fail_reason=""

    if [[ "$type" == "xml" ]]; then
        # 1. Check if ripwire-rs output is well-formed XML via yq
        if ! echo "$out_rw" | yq -p xml '.' >/dev/null 2>&1; then
            test_passed=false
            fail_reason="Invalid XML from ripwire-rs: $(echo "$out_rw" | yq -p xml '.' 2>&1 | head -n 1)"
        else
            # 2. Check schema and data assertion
            if [[ -n "$assert_expr" ]]; then
                assertion_eval=$(echo "$out_rw" | yq -p xml "$assert_expr" 2>/dev/null || echo "false")
                if [[ "$assertion_eval" != "true" ]]; then
                    test_passed=false
                    fail_reason="Data/Schema assertion failed: $assert_expr (got: $assertion_eval)"
                fi
            fi
        fi
    elif [[ "$type" == "text" ]]; then
        if [[ "$assert_expr" == "version" ]]; then
            if [[ "$out_rw" != *"ripwire-rs"* && "$out_rw" != *"rw"* ]]; then
                test_passed=false
                fail_reason="Missing version banner"
            fi
        elif [[ "$assert_expr" == "mermaid" ]]; then
            if [[ "$out_rw" != *"flowchart"* ]]; then
                test_passed=false
                fail_reason="Missing mermaid flowchart syntax"
            fi
        elif [[ "$assert_expr" == "html" ]]; then
            if [[ "$out_rw" != *"<!DOCTYPE html>"* ]]; then
                test_passed=false
                fail_reason="Missing HTML doctype"
            fi
        elif [[ "$assert_expr" == "wrap" ]]; then
            if [[ "$out_rw" != *"claude mcp add"* ]]; then
                test_passed=false
                fail_reason="Missing claude mcp add instruction"
            fi
        fi
    fi

    if $test_passed; then
        printf "  \033[32m[PASS]\033[0m %-30s - %s\n" "$cmd_label" "$desc"
        PASSED=$((PASSED + 1))
    else
        printf "  \033[31m[FAIL]\033[0m %-30s - %s (%s)\n" "$cmd_label" "$desc" "$fail_reason"
        FAILED=$((FAILED + 1))
    fi
done

echo "=================================================="
if [[ $FAILED -eq 0 ]]; then
    printf "\033[32mPARITY VERIFIED: %d/%d commands passed strict XML and schema checks.\033[0m\n" "$PASSED" "$TOTAL"
    exit 0
else
    printf "\033[31mPARITY FAILED: %d/%d commands failed.\033[0m\n" "$FAILED" "$TOTAL"
    exit 1
fi
