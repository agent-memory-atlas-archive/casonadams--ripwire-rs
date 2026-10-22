use crate::domain::{IngestResult, Interner, Lang, NodeId};
use crate::graph::Graph;
use std::collections::HashMap;
use std::io::{self, Write};

/// Node coloring mode for HTML visualization.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorBy {
    #[default]
    Lang = 0,
    Community = 1,
    Cx = 2,
    Churn = 3,
    Tested = 4,
}

impl ColorBy {
    #[must_use]
    pub fn from_str_opt(s: &str) -> Option<Self> {
        let lower = s.to_ascii_lowercase();
        if lower.starts_with("comm") {
            Some(Self::Community)
        } else if lower.starts_with('c') {
            if lower.contains('h') {
                Some(Self::Churn)
            } else {
                Some(Self::Cx)
            }
        } else if lower.starts_with('t') {
            Some(Self::Tested)
        } else if lower.starts_with('l') {
            Some(Self::Lang)
        } else {
            None
        }
    }

    #[must_use]
    pub fn mode_name(self) -> &'static str {
        match (self, 0) {
            (Self::Community, _) => "community",
            (Self::Cx, _) => "cx",
            (Self::Churn, _) => "churn",
            (Self::Tested, _) => "tested",
            _ => "lang",
        }
    }
}

impl std::fmt::Display for ColorBy {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.mode_name())
    }
}

impl std::str::FromStr for ColorBy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_opt(s).ok_or_else(|| format!("invalid color-by mode: '{s}'"))
    }
}
/// Palette of 12 distinct dark-theme-friendly community colors.
pub const COMMUNITY_COLORS: &[&str] = &[
    "#29b6f6", "#ab47bc", "#26a69a", "#ffa726", "#ec407a", "#7e57c2", "#66bb6a", "#ff7043",
    "#5c6bc0", "#8d6e63", "#26c6da", "#d4e157",
];

/// Language fill colors.
#[must_use]
pub fn lang_color(lang: Lang) -> &'static str {
    lang.semantics().display_color()
}

#[must_use]
pub fn cx_color(cx: u32) -> &'static str {
    if cx <= 5 {
        "#4caf50"
    } else if cx <= 10 {
        "#8bc34a"
    } else if cx <= 15 {
        "#ffeb3b"
    } else if cx <= 25 {
        "#ff9800"
    } else {
        "#f44336"
    }
}

struct HtmlNodeData {
    id: NodeId,
    file_id: u32,
    name: String,
    lang: String,
    cx: u32,
    comm: u32,
    rank: f64,
}

fn build_html_nodes(
    ingest: &IngestResult,
    ranks: &[f64],
    interner: &Interner,
    comms: &crate::graph::community::Communities,
    top_k: Option<usize>,
) -> (Vec<HtmlNodeData>, HashMap<NodeId, usize>) {
    let mut indices: Vec<usize> = (0..ingest.symbols.len()).collect();
    indices.sort_by(|&a, &b| {
        let ra = ranks.get(a).copied().unwrap_or(0.0);
        let rb = ranks.get(b).copied().unwrap_or(0.0);
        rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
    });

    let limit = top_k.unwrap_or(60).min(indices.len());

    let mut nodes = Vec::with_capacity(limit);
    let mut id_to_slot = HashMap::new();

    for (slot, &idx) in indices[..limit].iter().enumerate() {
        if let Some(sym) = ingest.symbols.get(idx) {
            let id = sym.id;
            let name = interner.resolve(&sym.name).to_string();
            let comm = comms.comm.get(id as usize).copied().unwrap_or(0);
            let rank = ranks.get(idx).copied().unwrap_or(0.0);

            nodes.push(HtmlNodeData {
                id,
                file_id: sym.file_id,
                name,
                lang: sym.lang.tag().to_string(),
                cx: sym.cx,
                comm,
                rank,
            });
            id_to_slot.insert(id, slot);
        }
    }

    (nodes, id_to_slot)
}

/// Generate a self-contained interactive call graph HTML string.
#[must_use]
pub fn generate_call_graph_html(
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
    initial_color_by: ColorBy,
    top_k: Option<usize>,
) -> String {
    let comms = graph.communities();
    let (nodes, id_to_slot) = build_html_nodes(ingest, ranks, interner, &comms, top_k);
    let mut files_json = String::from("const FILES = [\n");
    for (i, f) in ingest.files.iter().enumerate() {
        let p = interner.resolve(&f.path);
        files_json.push_str(&format!(
            "  {:?}{}\n",
            p,
            if i + 1 < ingest.files.len() { "," } else { "" }
        ));
    }
    files_json.push_str("];\n");

    let mut nodes_json = String::from("const NODES = [\n");
    for (k, n) in nodes.iter().enumerate() {
        nodes_json.push_str(&format!(
            "  {{\"id\":{},\"label\":{:?},\"type\":\"fn\",\"lang\":{:?},\"rank\":{:.4},\"file\":{},\"comm\":{},\"cx\":{},\"ts\":0}}{}\n",
            k, n.name, n.lang, n.rank, n.file_id, n.comm, n.cx,
            if k + 1 < nodes.len() { "," } else { "" }
        ));
    }
    nodes_json.push_str("];\n");

    let mut links_json = String::from("const LINKS = [\n");
    let mut edges = Vec::new();
    for &src_slot in id_to_slot.values() {
        let src_node = &nodes[src_slot];
        let amb = graph.ambiguity(src_node.id) > 0;
        for (tgt_id, _) in graph.callees(src_node.id) {
            if let Some(&tgt_slot) = id_to_slot.get(&tgt_id) {
                edges.push((src_slot, tgt_slot, amb));
            }
        }
    }
    edges.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    edges.dedup();
    for (i, (s, t, amb)) in edges.iter().enumerate() {
        links_json.push_str(&format!(
            "  {{\"s\":{},\"t\":{}{}}}{}\n",
            s,
            t,
            if *amb { ",\"a\":1" } else { "" },
            if i + 1 < edges.len() { "," } else { "" }
        ));
    }
    links_json.push_str("];\n");

    let mut modules_json = String::from("const MODULES = [\n");
    let mut comm_members: HashMap<u32, Vec<usize>> = HashMap::new();
    for (k, n) in nodes.iter().enumerate() {
        comm_members.entry(n.comm).or_default().push(k);
    }
    let mut comm_list: Vec<(u32, Vec<usize>)> = comm_members.into_iter().collect();
    comm_list.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    for (disp, (comm_id, members)) in comm_list.iter().enumerate() {
        let first_file = members.first().map(|&idx| nodes[idx].file_id).unwrap_or(0);
        let top_m = &members[..members.len().min(5)];
        modules_json.push_str(&format!(
            "  {{\"id\":{},\"name\":{:?},\"symCount\":{},\"fileCount\":1,\"files\":[{}],\"members\":{:?},\"top\":{:?},\"inCross\":0,\"outCross\":0,\"neigh\":[]}}{}\n",
            disp, format!("module_{}", comm_id), members.len(), first_file, members, top_m,
            if disp + 1 < comm_list.len() { "," } else { "" }
        ));
    }
    modules_json.push_str("];\n");

    const HTML_VIS_JS: &str = include_str!("html_vis.js");
    let lang_colors = crate::languages::ALL_LANGS
        .iter()
        .filter_map(|lang| Some(format!("{:?}: {:?}", lang.tag(), lang.semantics().color()?)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>ripwire graph</title>
<style>
* {{ margin:0; padding:0; box-sizing:border-box; }}
body {{ background:#111; color:#eee; font:13px/1.4 sans-serif; overflow:hidden; }}
#bar {{ position:fixed; top:0; left:0; right:0; height:36px; background:rgba(0,0,0,.7); display:flex; align-items:center; gap:12px; padding:0 12px; z-index:10; border-bottom:1px solid #333; }}
#bar a {{ color:#7fb2ff; text-decoration:none; font-size:12px; }}
#bar input {{ background:#222; border:1px solid #444; color:#eee; padding:3px 8px; border-radius:4px; font-size:12px; }}
#bar select {{ background:#222; border:1px solid #444; color:#eee; padding:3px 6px; border-radius:4px; font-size:12px; }}
#c {{ width:100vw; height:100vh; display:block; }}
.module-card {{ data-module-card:1; }}
#cards {{ display:none; padding:48px 16px 16px; overflow:auto; height:100vh; grid-template-columns:repeat(auto-fill,minmax(260px,1fr)); gap:12px; }}
#crumb {{ position:fixed; bottom:12px; left:12px; background:rgba(0,0,0,.8); padding:4px 8px; border-radius:4px; font-size:11px; z-index:10; }}
#tooltip {{ position:absolute; display:none; background:#222; border:1px solid #666; padding:6px 10px; border-radius:4px; font-size:11px; pointer-events:none; z-index:20; }}
</style>
</head>
<body>
<div id="bar">
  <h1>ripwire graph</h1>
  <a class="nav" href="#graph">Graph</a>
  <a class="nav" href="#overview">Modules</a>
  <input id="search" type="text" placeholder="search labels...">
  <span id="depth">depth <input id="depthSlider" type="range" min="1" max="3" value="2" style="width:60px"><span id="depthVal">2</span></span>
  <span id="info"></span>
  <select id="colorMode"><option value="lang">lang</option><option value="community">community</option><option value="cx">cx</option><option value="churn">churn</option><option value="tested">tested</option></select>
  <span id="legend">read the source before trusting one</span>
  <button id="savePng" title="download this view as a PNG">PNG</button>
</div>
<div id="prov"></div>
<div id="hits"></div>
<div id="crumb"></div>
<div id="cards" data-module-card="1"></div>
<canvas id="c"></canvas>
<div id="tooltip"></div>
<script>
const ROOT = ".";
const AT_STAMP = "";
const BINARY_VER = "0.3.1";
const TOPK = {};
const NODE_TOTAL = {};
const EDGE_TOTAL = {};
const SYM_TOTAL = {};
const COLOR_MODE = "{}";
const LANG_COLORS = {{ {} }};
const SHAPES = {{}};
const CHURN_MAP = [];
const HAS_CHURN = false;
{}
{}
{}
{}
{}
</script>
</body>
</html>"##,
        top_k.unwrap_or(5000),
        nodes.len(),
        edges.len(),
        nodes.len(),
        initial_color_by.mode_name(),
        lang_colors,
        files_json,
        nodes_json,
        links_json,
        modules_json,
        HTML_VIS_JS
    )
}

/// Write call graph HTML directly to a stream.
pub fn write_call_graph_html<W: Write>(
    out: &mut W,
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
    color_by: ColorBy,
    top_k: Option<usize>,
) -> io::Result<()> {
    let html = generate_call_graph_html(ingest, graph, ranks, interner, color_by, top_k);
    out.write_all(html.as_bytes())
}
