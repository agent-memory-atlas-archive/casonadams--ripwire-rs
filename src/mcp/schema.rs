use serde_json::{Value, json};

/// Return list of all 33 MCP tools plus aliases.
#[must_use]
pub fn all_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "affected",
            "description": "WHICH TESTS TO RUN for a change — test files that transitively reach the changed files/symbols",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "files": { "type": "string", "description": "Changed files or symbols, comma-separated" }
                },
                "required": ["files"]
            }
        }),
        json!({
            "name": "analyze",
            "description": "Architecture map for a directory: top symbols, signatures, and direct callees",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "top_k": { "type": "integer", "description": "Maximum number of symbols" },
                    "rank_by": { "type": "string", "description": "Signal to rank by: pagerank, degree, ccx, churn" }
                }
            }
        }),
        json!({
            "name": "batch",
            "description": "ONE-TURN CONTEXT SWEEP: answer up to 16 heterogeneous READ sub-queries in a single call",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "queries": { "type": "array", "description": "Array of tool call objects" }
                },
                "required": ["queries"]
            }
        }),
        json!({
            "name": "cochange",
            "description": "Files that historically change together with this file (Fowler's Shotgun Surgery logical coupling)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Target file path to find co-change partners for" },
                    "since": { "type": "string", "description": "Time window or revision (e.g. '18 months ago')" }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "connect",
            "description": "When a task touches 2..16 named symbols, returns the minimal subgraph relating them",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbols": { "type": "array", "items": { "type": "string" }, "description": "List of symbol names" }
                },
                "required": ["symbols"]
            }
        }),
        json!({
            "name": "doc_drift",
            "description": "WHICH OF THIS REPO'S DOC CLAIMS ARE NOW FALSE: verifies checkable anchors in markdown against live index",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "edit_check",
            "description": "After an edit: did SYM's contract change, and which callers no longer fit?",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "Symbol name to verify" }
                },
                "required": ["symbol"]
            }
        }),
        json!({
            "name": "exemplar",
            "description": "BEFORE writing a function/method/class/struct, get repo's single best-in-class instance to imitate",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "role": { "type": "string", "description": "Kind of symbol: fn, method, class, struct, interface" }
                }
            }
        }),
        json!({
            "name": "explore",
            "description": "ONE-call task orientation: ranked symbols, files, and architecture map",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Repository path to explore" },
                    "top_k": { "type": "integer", "description": "Maximum number of symbols to include" }
                }
            }
        }),
        json!({
            "name": "fetch_body",
            "description": "Full (or partial-range) source of a symbol's definition",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "Symbol name" },
                    "path": { "type": "string", "description": "Optional file path hint" },
                    "start_line": { "type": "integer", "description": "Optional 1-based start line" },
                    "end_line": { "type": "integer", "description": "Optional 1-based end line" }
                },
                "required": ["symbol"]
            }
        }),
        json!({
            "name": "find_referencing_symbols",
            "description": "Direct (1-hop) callers of a symbol",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "Symbol name to look up" }
                },
                "required": ["symbol"]
            }
        }),
        json!({
            "name": "find_symbol",
            "description": "A symbol's 1-hop neighborhood: the symbol plus direct callers and callees",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "Symbol name to look up" }
                },
                "required": ["symbol"]
            }
        }),
        json!({
            "name": "flags",
            "description": "WHAT IS BUILT BUT DARK: harvests ifndef header gates, CMake options, env reads",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "for",
            "description": "Task-lens ranked signatures-only inventory of building blocks most relevant to a task",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "Task description in plain words" },
                    "budget_tokens": { "type": "integer", "description": "Maximum number of symbols" }
                },
                "required": ["task"]
            }
        }),
        json!({
            "name": "from_trace",
            "description": "Paste a stack trace / compiler error and get it mapped onto indexed symbols innermost-first",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "trace": { "type": "string", "description": "Stack trace text" }
                },
                "required": ["trace"]
            }
        }),
        json!({
            "name": "grep",
            "description": "Literal search; each hit annotated with its enclosing symbol",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Literal pattern to search" },
                    "path": { "type": "string", "description": "Optional path filter" }
                },
                "required": ["pattern"]
            }
        }),
        json!({
            "name": "impact",
            "description": "Transitive blast radius of a symbol via calls, ranked by PageRank",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "Symbol name to analyze blast radius for" },
                    "max_depth": { "type": "integer", "description": "Maximum hop depth" }
                },
                "required": ["symbol"]
            }
        }),
        json!({
            "name": "insert_after_symbol",
            "description": "Insert text immediately AFTER a symbol's definition (atomic write-rename)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "Symbol name" },
                    "text": { "type": "string", "description": "Text to insert" },
                    "path": { "type": "string", "description": "Optional file path hint" }
                },
                "required": ["symbol", "text"]
            }
        }),
        json!({
            "name": "insert_before_symbol",
            "description": "Insert text immediately BEFORE a symbol's definition (atomic write-rename)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "Symbol name" },
                    "text": { "type": "string", "description": "Text to insert" },
                    "path": { "type": "string", "description": "Optional file path hint" }
                },
                "required": ["symbol", "text"]
            }
        }),
        json!({
            "name": "lego",
            "description": "Interface-to-impls view: method contract plus EVERY implementor",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "interface": { "type": "string", "description": "Interface, trait or base class name" }
                },
                "required": ["interface"]
            }
        }),
        json!({
            "name": "memory_recall",
            "description": "Most relevant memory notes / docs for a task, full text",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "Task description" },
                    "top_k": { "type": "integer", "description": "Maximum number of notes" }
                },
                "required": ["task"]
            }
        }),
        json!({
            "name": "mentions",
            "description": "Docs that name a code symbol in a backtick",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "Symbol name" }
                },
                "required": ["symbol"]
            }
        }),
        json!({
            "name": "owners",
            "description": "Bus-factor: recency-weighted author ownership per file",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Optional file path filter" }
                }
            }
        }),
        json!({
            "name": "path_between",
            "description": "Does A REACH B, and HOW? Shortest directed call path between two symbols",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from": { "type": "string", "description": "Source symbol name" },
                    "to": { "type": "string", "description": "Target symbol name" }
                },
                "required": ["from", "to"]
            }
        }),
        json!({
            "name": "quality_baseline",
            "description": "PIN the quality floor: writes .ripwire_quality_baseline stamped with git HEAD sha",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "quality_delta",
            "description": "Your PR self-check: reports ONLY what your working tree made WORSE vs baseline",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "rank_by",
            "description": "Architecture map ranked by a signal: pagerank, degree, ccx, churn",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "rank_by": { "type": "string", "description": "pagerank, degree, ccx, churn" },
                    "top_k": { "type": "integer", "description": "Maximum symbols to include" }
                },
                "required": ["rank_by"]
            }
        }),
        json!({
            "name": "replace_symbol_body",
            "description": "Replace a symbol's ENTIRE definition with new_body (atomic write-rename)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "Symbol name" },
                    "new_body": { "type": "string", "description": "Replacement definition" },
                    "path": { "type": "string", "description": "Optional file path hint" }
                },
                "required": ["symbol", "new_body"]
            }
        }),
        json!({
            "name": "situational_awareness",
            "description": "The 5 things to know about a diff: blast_radius, tests_to_run, forgotten co-change files, hotspots",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "files": { "type": "array", "items": { "type": "string" }, "description": "Changed file paths" }
                },
                "required": ["files"]
            }
        }),
        json!({
            "name": "slice",
            "description": "ARISE intra-procedural def-use variable dataflow slice inside a function",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "sym:var or bare sym to list locals" },
                    "flow": { "type": "string", "description": "fwd, back, or both" }
                },
                "required": ["target"]
            }
        }),
        json!({
            "name": "stray_content",
            "description": "Lines authored on divergent branches that HEAD does not have",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "uses",
            "description": "All statically resolvable use-sites of a symbol: call, read, write, import, extends",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "Symbol name" }
                },
                "required": ["symbol"]
            }
        }),
        json!({
            "name": "whereis",
            "description": "WHERE DOES THIS CONTENT LIVE: which file/branch defines or mentions a symbol",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "Symbol name" }
                },
                "required": ["symbol"]
            }
        }),
        json!({
            "name": "callers",
            "description": "Direct (1-hop) callers of a symbol (alias for find_referencing_symbols)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "Symbol name to look up" }
                },
                "required": ["symbol"]
            }
        }),
        json!({
            "name": "callees",
            "description": "Direct (1-hop) callees of a symbol",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "Symbol name to look up" }
                },
                "required": ["symbol"]
            }
        }),
    ]
}
