# Model Context Protocol (MCP)

`ripwire` provides a native stdio Model Context Protocol (MCP) server conforming to JSON-RPC 2.0.

## Client Registration

### Claude Desktop / OpenCode / Omp (`claude_desktop_config.json`)

```json
{
  "mcpServers": {
    "ripwire-rs": {
      "command": "/path/to/ripwire-rs",
      "args": ["mcp"]
    }
  }
}
```

---

## Complete 33 Tools Inventory

| Tool | Category | Description |
| :--- | :--- | :--- |
| `explore` | Retrieval | Ranked map, bodies, callers, and test targets in a single byte-budgeted response. |
| `for` | Retrieval | Task-lens relevance ranking using BM25 and Personalized PageRank. |
| `batch` | Retrieval | Execute up to 16 heterogeneous sub-queries in a single call. |
| `fetch_body` | Retrieval | Fetch symbol definition source code by handle or name with optional line bounds. |
| `exemplar` | Retrieval | Retrieve the best-in-class implementation of a given role (fn, class, interface). |
| `lego` | Retrieval | Interface contract view with all implementors across the workspace. |
| `grep` | Search | Trigram literal search annotated with enclosing symbols and definitions. |
| `find_symbol` | Navigation | 1-hop symbol neighborhood: callers, callees, definitions, and ambiguity metrics. |
| `find_referencing_symbols` | Navigation | Direct (1-hop) callers of a symbol or database table. |
| `impact` | Navigation | Transitive blast radius of a symbol or database table via call hierarchy. |
| `uses` | Navigation | Statically resolvable use-sites (call, read, write, import, extends), including embedded SQL table lineage from application strings. |
| `path_between` | Architecture | Shortest directed call path between two symbols. |
| `connect` | Architecture | Minimal directed connecting subgraph relating 2..16 symbols. |
| `analyze` | Architecture | Architecture map ranked by PageRank or chosen metric. |
| `rank_by` | Architecture | Architecture map ranked by alternate signal (authority, hub, churn). |
| `situational_awareness` | Architecture | Diff context: blast radius, tests to run, and forgotten co-change partners. |
| `from_trace` | Debugging | Maps stack trace frames and compiler errors onto indexed symbol definitions. |
| `slice` | Slicing | Intra-procedural def-use variable slicing (forward, backward, or both). |
| `cochange` | Git Analytics | Historical Shotgun Surgery co-change coupling. |
| `owners` | Git Analytics | Recency-weighted author ownership per file. |
| `edit_check` | Safety Gates | Verify caller compatibility after editing a function contract. |
| `affected` | Safety Gates | Transitive test reachability mapping from changed symbols. |
| `quality_baseline` | Safety Gates | Snapshot code quality floor at current commit. |
| `quality_delta` | Safety Gates | Pre-commit quality regression self-check against baseline or git HEAD. |
| `doc_drift` | Documentation | Detect out-of-bounds file:line references in markdown documents. |
| `mentions` | Documentation | Locate markdown design notes referencing a code symbol in backticks. |
| `memory_recall` | Documentation | Relevant documentation and design notes matching a task. |
| `flags` | Inspection | Dark feature flags, `#ifdef` guards, and runtime environment variable reads. |
| `whereis` | Inspection | Branch divergence tracker: finds which branch's tree defines a symbol. |
| `stray_content` | Inspection | Lines authored on a divergent branch not present in HEAD. |
| `replace_symbol_body` | Atomic Edit | Atomically replace an entire symbol definition with verified byte rollback. |
| `insert_before_symbol` | Atomic Edit | Atomically insert source code before a symbol definition. |
| `insert_after_symbol` | Atomic Edit | Atomically insert source code after a symbol definition. |
