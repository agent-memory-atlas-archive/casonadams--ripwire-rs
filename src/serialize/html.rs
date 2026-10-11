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

/// Palette of 12 distinct dark-theme-friendly community colors.
pub const COMMUNITY_COLORS: &[&str] = &[
    "#29b6f6", "#ab47bc", "#26a69a", "#ffa726", "#ec407a", "#7e57c2", "#66bb6a", "#ff7043",
    "#5c6bc0", "#8d6e63", "#26c6da", "#d4e157",
];

/// Language fill colors.
#[must_use]
pub fn lang_color(lang: Lang) -> &'static str {
    const COLORS: [&str; 25] = [
        "#dea584", "#3178c6", "#f1e05a", "#3572a5", "#00add8", "#555555", "#f34b7d", "#3a87ad",
        "#3a87ad", "#b07219", "#178600", "#f05138", "#89e051", "#701516", "#4f5d95", "#000080",
        "#6e4a7e", "#00b4ab", "#a97bff", "#355570", "#292929", "#9c4221", "#cb171e", "#083fa1",
        "#888888",
    ];
    let idx = lang as usize;
    if idx < COLORS.len() {
        COLORS[idx]
    } else {
        "#888888"
    }
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
    name: String,
    path: String,
    line: u32,
    lang: String,
    cx: u32,
    comm: u32,
    rank: f64,
    x: f64,
    y: f64,
}

fn compute_node_coordinates(count: usize) -> Vec<(f64, f64)> {
    let mut coords = Vec::with_capacity(count);
    let center_x = 500.0;
    let center_y = 400.0;
    let radius = 320.0;

    for i in 0..count {
        if count == 1 {
            coords.push((center_x, center_y));
        } else {
            let angle = (i as f64) * (2.0 * std::f64::consts::PI) / (count as f64);
            let r = radius * (0.4 + 0.6 * ((i % 3) as f64 / 2.0));
            coords.push((center_x + r * angle.cos(), center_y + r * angle.sin()));
        }
    }
    coords
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
    let coords = compute_node_coordinates(limit);

    let mut nodes = Vec::with_capacity(limit);
    let mut id_to_slot = HashMap::new();

    for (slot, &idx) in indices[..limit].iter().enumerate() {
        if let Some(sym) = ingest.symbols.get(idx) {
            let id = sym.id;
            let name = interner.resolve(&sym.name).to_string();
            let file_path = ingest
                .files
                .get(sym.file_id as usize)
                .map(|f| interner.resolve(&f.path).to_string())
                .unwrap_or_default();
            let comm = comms.comm.get(id as usize).copied().unwrap_or(0);
            let rank = ranks.get(idx).copied().unwrap_or(0.0);
            let (x, y) = coords[slot];

            nodes.push(HtmlNodeData {
                id,
                name,
                path: file_path,
                line: sym.line,
                lang: sym.lang.tag().to_string(),
                cx: sym.cx,
                comm,
                rank,
                x,
                y,
            });
            id_to_slot.insert(id, slot);
        }
    }

    (nodes, id_to_slot)
}

fn render_svg_elements(
    nodes: &[HtmlNodeData],
    id_to_slot: &HashMap<NodeId, usize>,
    graph: &Graph,
) -> (String, String) {
    let mut links_svg = String::new();
    for node in nodes {
        for (callee, _) in graph.callees(node.id) {
            if let Some(&target_slot) = id_to_slot.get(&callee) {
                let target = &nodes[target_slot];
                links_svg.push_str(&format!(
                    "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#444\" stroke-width=\"1.2\" marker-end=\"url(#arrow)\" />\n",
                    node.x, node.y, target.x, target.y
                ));
            }
        }
    }

    let mut nodes_svg = String::new();
    for node in nodes {
        let r = (6.0 + (node.rank * 120.0)).clamp(6.0, 24.0);
        let color = COMMUNITY_COLORS[(node.comm as usize) % COMMUNITY_COLORS.len()];
        nodes_svg.push_str(&format!(
            "<g class=\"node\" data-id=\"{}\" data-name=\"{}\" data-lang=\"{}\" data-comm=\"{}\" data-cx=\"{}\" transform=\"translate({:.1},{:.1})\">\n",
            node.id, node.name, node.lang, node.comm, node.cx, node.x, node.y
        ));
        nodes_svg.push_str(&format!(
            "  <circle r=\"{:.1}\" fill=\"{}\" stroke=\"#fff\" stroke-width=\"1.5\" />\n",
            r, color
        ));
        nodes_svg.push_str(&format!(
            "  <text dx=\"{:.1}\" dy=\"4\" fill=\"#e0e0e0\" font-size=\"11\">{}</text>\n",
            r + 4.0,
            node.name
        ));
        nodes_svg.push_str("</g>\n");
    }

    (links_svg, nodes_svg)
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
    let (links_svg, nodes_svg) = render_svg_elements(&nodes, &id_to_slot, graph);

    let nodes_json = serde_json::to_string(
        &nodes
            .iter()
            .map(|n| {
                serde_json::json!({
                    "id": n.id,
                    "name": n.name,
                    "path": n.path,
                    "line": n.line,
                    "lang": n.lang,
                    "cx": n.cx,
                    "comm": n.comm,
                    "rank": (n.rank * 10000.0).round() / 10000.0,
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>rwire Call Graph</title>
<style>
  body {{ margin: 0; background: #121212; color: #e0e0e0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, monospace; overflow: hidden; }}
  header {{ display: flex; align-items: center; gap: 12px; padding: 10px 16px; background: #1e1e1e; border-bottom: 1px solid #333; }}
  select, input {{ background: #2a2a2a; color: #fff; border: 1px solid #555; padding: 6px 10px; border-radius: 4px; font-size: 13px; }}
  #canvas-container {{ width: 100vw; height: calc(100vh - 52px); cursor: grab; }}
  svg {{ width: 100%; height: 100%; display: block; }}
  .node:hover circle {{ stroke: #ffeb3b; stroke-width: 2.5; cursor: pointer; }}
  #tooltip {{ position: absolute; display: none; background: #222; border: 1px solid #666; padding: 8px 12px; border-radius: 6px; font-size: 12px; pointer-events: none; z-index: 10; }}
</style>
</head>
<body>
<header>
  <strong>rwire</strong>
  <label>Color by:
    <select id="colorMode">
      <option value="lang">Language</option>
      <option value="community">Community</option>
      <option value="cx">Complexity (cx)</option>
    </select>
  </label>
  <input type="text" id="search" placeholder="Search symbol...">
  <span style="font-size: 12px; color: #888;">Nodes: {} | Edges: {}</span>
</header>
<div id="tooltip"></div>
<div id="canvas-container">
  <svg id="svgRoot" viewBox="0 0 1000 800">
    <defs>
      <marker id="arrow" viewBox="0 -5 10 10" refX="20" refY="0" markerWidth="6" markerHeight="6" orient="auto">
        <path d="M0,-5L10,0L0,5" fill="#666" />
      </marker>
    </defs>
    <g id="viewport">
      <g id="links">{}</g>
      <g id="nodes">{}</g>
    </g>
  </svg>
</div>
<script>
const NODES_DATA = {};
const COMM_COLORS = {:?};
const LANG_COLORS = {{
  "rs": "#dea584", "ts": "#3178c6", "js": "#f1e05a", "py": "#3572a5", "go": "#00add8",
  "c": "#555", "cpp": "#f34b7d", "java": "#b07219", "cs": "#178600", "swift": "#f05138"
}};
let activeMode = "{}";

function cxColor(cx) {{
  if (cx <= 5) return "#4caf50";
  if (cx <= 10) return "#8bc34a";
  if (cx <= 15) return "#ffeb3b";
  if (cx <= 25) return "#ff9800";
  return "#f44336";
}}

function updateNodeColors(mode) {{
  document.querySelectorAll(".node").forEach(g => {{
    const id = parseInt(g.dataset.id, 10);
    const n = NODES_DATA.find(x => x.id === id);
    if (!n) return;
    let fill = "#888";
    if (mode === "community") fill = COMM_COLORS[n.comm % COMM_COLORS.length];
    else if (mode === "lang") fill = LANG_COLORS[n.lang] || "#888";
    else if (mode === "cx") fill = cxColor(n.cx);
    g.querySelector("circle").setAttribute("fill", fill);
  }});
}}

document.getElementById("colorMode").value = activeMode;
document.getElementById("colorMode").addEventListener("change", e => {{
  activeMode = e.target.value;
  updateNodeColors(activeMode);
}});

updateNodeColors(activeMode);

// Tooltip logic
const tip = document.getElementById("tooltip");
document.querySelectorAll(".node").forEach(g => {{
  g.addEventListener("mouseenter", e => {{
    const id = parseInt(g.dataset.id, 10);
    const n = NODES_DATA.find(x => x.id === id);
    if (!n) return;
    tip.style.display = "block";
    tip.innerHTML = `<strong>${{n.name}}</strong><br>${{n.path}}:${{n.line}}<br>Rank: ${{n.rank}} | cx: ${{n.cx}} | comm: ${{n.comm}}`;
  }});
  g.addEventListener("mousemove", e => {{
    tip.style.left = (e.pageX + 12) + "px";
    tip.style.top = (e.pageY + 12) + "px";
  }});
  g.addEventListener("mouseleave", () => {{ tip.style.display = "none"; }});
}});

// Search highlight
document.getElementById("search").addEventListener("input", e => {{
  const q = e.target.value.toLowerCase();
  document.querySelectorAll(".node").forEach(g => {{
    const match = !q || g.dataset.name.toLowerCase().includes(q);
    g.style.opacity = match ? "1" : "0.15";
  }});
}});
</script>
</body>
</html>"##,
        nodes.len(),
        graph.edge_count(),
        links_svg,
        nodes_svg,
        nodes_json,
        COMMUNITY_COLORS,
        initial_color_by.mode_name()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{SymKind, Symbol};

    #[test]
    fn test_color_by_parsing() {
        assert_eq!(ColorBy::from_str_opt("lang"), Some(ColorBy::Lang));
        assert_eq!(ColorBy::from_str_opt("community"), Some(ColorBy::Community));
        assert_eq!(ColorBy::from_str_opt("cx"), Some(ColorBy::Cx));
        assert_eq!(ColorBy::from_str_opt("churn"), Some(ColorBy::Churn));
        assert_eq!(ColorBy::from_str_opt("tested"), Some(ColorBy::Tested));
        assert_eq!(ColorBy::from_str_opt("invalid"), None);

        assert_eq!(ColorBy::Lang.mode_name(), "lang");
        assert_eq!(ColorBy::Community.mode_name(), "community");
        assert_eq!(ColorBy::Cx.mode_name(), "cx");
        assert_eq!(ColorBy::Churn.mode_name(), "churn");
        assert_eq!(ColorBy::Tested.mode_name(), "tested");
    }

    #[test]
    fn test_colors_lang_and_cx() {
        assert_eq!(lang_color(Lang::Rust), "#dea584");
        assert_eq!(lang_color(Lang::Python), "#3572a5");
        assert_eq!(lang_color(Lang::Unknown), "#888888");

        assert_eq!(cx_color(3), "#4caf50");
        assert_eq!(cx_color(8), "#8bc34a");
        assert_eq!(cx_color(14), "#ffeb3b");
        assert_eq!(cx_color(20), "#ff9800");
        assert_eq!(cx_color(50), "#f44336");
    }

    #[test]
    fn test_generate_and_write_call_graph_html() {
        let interner = Interner::new();
        let mut ingest = IngestResult::new();
        let f_spur = interner.intern("src/main.rs");
        let n1_spur = interner.intern("main");
        let n2_spur = interner.intern("helper");
        ingest.symbols.push(Symbol::new(
            0,
            f_spur,
            n1_spur,
            SymKind::Function,
            Lang::Rust,
            0,
            1,
        ));
        ingest.symbols.push(Symbol::new(
            1,
            f_spur,
            n2_spur,
            SymKind::Function,
            Lang::Rust,
            0,
            10,
        ));

        let graph = Graph::build(&ingest, &interner);
        let ranks = vec![0.6, 0.4];

        let mut buf = Vec::new();
        write_call_graph_html(
            &mut buf,
            &ingest,
            &graph,
            &ranks,
            &interner,
            ColorBy::Community,
            Some(10),
        )
        .unwrap();

        let html = String::from_utf8(buf).unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<svg"));
        assert!(html.contains("Nodes: 2"));
    }
}
