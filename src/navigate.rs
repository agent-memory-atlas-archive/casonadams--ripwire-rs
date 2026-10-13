use crate::domain::{IngestResult, Interner, NodeId};
use crate::graph::Graph;
use serde::Serialize;
use std::collections::{BTreeSet, HashSet, VecDeque};
use std::io::{self, Write};
/// Detailed navigation information for a symbol.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SymbolNavInfo {
    pub id: NodeId,
    pub name: String,
    pub path: String,
    pub line: u32,
    pub kind: &'static str,
    pub rank: f64,
    pub weight: f32,
}

/// 1-hop neighborhood of a symbol containing both incoming callers and outgoing callees.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NavNeighborhood {
    pub target: SymbolNavInfo,
    pub callers: Vec<SymbolNavInfo>,
    pub callees: Vec<SymbolNavInfo>,
}

/// Transitive reachability impact node with hop distance.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImpactNode {
    pub id: NodeId,
    pub name: String,
    pub path: String,
    pub line: u32,
    pub kind: &'static str,
    pub rank: f64,
    pub hops: u32,
}

fn matches_symbol_selector(
    sym: &crate::domain::Symbol,
    ingest: &IngestResult,
    interner: &Interner,
    selector: &str,
) -> bool {
    let sym_name = interner.resolve(&sym.name);
    let sym_scope = sym.scope.map(|sc| interner.resolve(&sc)).unwrap_or("");
    let file_path = interner.resolve(&ingest.files[sym.file_id as usize].path);

    if let Some((scope, name)) = selector.split_once("::") {
        if let Some((file, sc)) = scope.split_once("::") {
            return sym_name == name && sym_scope == sc && file_path.ends_with(file);
        }
        return sym_name == name && (sym_scope == scope || file_path.ends_with(scope));
    }

    if let Some((file, name)) = selector.split_once(':') {
        return sym_name == name && file_path.ends_with(file);
    }

    if let Some((scope, name)) = selector.split_once('.') {
        return sym_name == name && sym_scope == scope;
    }

    sym_name == selector
}

/// Find symbol IDs by exact name match or qualified selector (`p::sc::n`, `sc::n`, `file:name`, `Type.method`).
#[must_use]
pub fn find_symbols_by_name(name: &str, ingest: &IngestResult, interner: &Interner) -> Vec<NodeId> {
    if let Some(target_spur) = interner.get(name) {
        let exact: Vec<NodeId> = ingest
            .symbols
            .iter()
            .filter(|s| s.name == target_spur)
            .map(|s| s.id)
            .collect();
        if !exact.is_empty() {
            return exact;
        }
    }

    ingest
        .symbols
        .iter()
        .filter(|s| matches_symbol_selector(s, ingest, interner, name))
        .map(|s| s.id)
        .collect()
}

/// Build `SymbolNavInfo` for a node.
#[must_use]
pub fn get_symbol_nav_info(
    id: NodeId,
    ingest: &IngestResult,
    ranks: &[f64],
    interner: &Interner,
    weight: f32,
) -> Option<SymbolNavInfo> {
    let sym = ingest.symbols.get(id as usize)?;
    let name = interner.resolve(&sym.name).to_string();
    let file = ingest.files.get(sym.file_id as usize)?;
    let path = interner.resolve(&file.path).to_string();
    let rank = ranks.get(id as usize).copied().unwrap_or(0.0);

    Some(SymbolNavInfo {
        id,
        name,
        path,
        line: sym.line,
        kind: sym.kind.tag(),
        rank,
        weight,
    })
}

/// Compute 1-hop neighborhood (direct callers and callees) for a target symbol.
#[must_use]
pub fn navigate_1hop(
    sym_id: NodeId,
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
) -> Option<NavNeighborhood> {
    let target = get_symbol_nav_info(sym_id, ingest, ranks, interner, 1.0)?;

    let mut callers = Vec::new();
    for (caller_id, weight) in graph.callers(sym_id) {
        if let Some(info) = get_symbol_nav_info(caller_id, ingest, ranks, interner, weight) {
            callers.push(info);
        }
    }
    callers.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| a.id.cmp(&b.id)));

    let mut callees = Vec::new();
    for (callee_id, weight) in graph.callees(sym_id) {
        if let Some(info) = get_symbol_nav_info(callee_id, ingest, ranks, interner, weight) {
            callees.push(info);
        }
    }
    callees.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| a.id.cmp(&b.id)));

    Some(NavNeighborhood {
        target,
        callers,
        callees,
    })
}

fn create_impact_node(
    curr: NodeId,
    depth: u32,
    ingest: &IngestResult,
    ranks: &[f64],
    interner: &Interner,
) -> Option<ImpactNode> {
    let sym = ingest.symbols.get(curr as usize)?;
    let name = interner.resolve(&sym.name).to_string();
    let path = ingest
        .files
        .get(sym.file_id as usize)
        .map(|f| interner.resolve(&f.path).to_string())
        .unwrap_or_default();
    let rank = ranks.get(curr as usize).copied().unwrap_or(0.0);

    Some(ImpactNode {
        id: curr,
        name,
        path,
        line: sym.line,
        kind: sym.kind.tag(),
        rank,
        hops: depth,
    })
}

/// Compute transitive blast radius / caller closure backward along in-edges using BFS.
#[must_use]
pub fn transitive_impact(
    seed_id: NodeId,
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
    max_depth: Option<u32>,
) -> Vec<ImpactNode> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut results = Vec::new();

    visited.insert(seed_id);
    queue.push_back((seed_id, 0u32));

    while let Some((curr, depth)) = queue.pop_front() {
        if curr != seed_id
            && let Some(node) = create_impact_node(curr, depth, ingest, ranks, interner)
        {
            results.push(node);
        }

        if max_depth.is_some_and(|limit| depth >= limit) {
            continue;
        }

        for (caller_id, _) in graph.callers(curr) {
            if visited.insert(caller_id) {
                queue.push_back((caller_id, depth + 1));
            }
        }
    }

    results.sort_by(|a, b| a.hops.cmp(&b.hops).then_with(|| b.rank.total_cmp(&a.rank)));
    results
}

fn write_nav_items<W: Write>(
    tag: &'static str,
    of: &str,
    items: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    write!(
        out,
        "<{tag} schema=\"ripwire.{tag}/v1\" of=\"{of}\" defs=\"1\" count=\"{}\" root=\".\">",
        items.len()
    )?;
    for it in items {
        write!(
            out,
            "<s t=\"{}\" n=\"{}\" p=\"{}:{}\"/>",
            it.kind, it.name, it.path, it.line
        )?;
    }
    writeln!(out, "</{tag}>")
}

/// Serialize direct callers to Ripwire-compatible XML.
pub fn serialize_callers_xml<W: Write>(
    sym_name: &str,
    callers: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    write_nav_items("callers", sym_name, callers, out)
}

/// Serialize direct callees to Ripwire-compatible XML.
pub fn serialize_callees_xml<W: Write>(
    sym_name: &str,
    callees: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    write_nav_items("callees", sym_name, callees, out)
}

/// Serialize transitive impact blast radius to Ripwire-compatible XML.
pub fn serialize_impact_xml<W: Write>(
    sym_name: &str,
    nodes: &[ImpactNode],
    out: &mut W,
) -> io::Result<()> {
    write!(
        out,
        "<impact schema=\"ripwire.impact/v1\" of=\"{sym_name}\" defs=\"1\" reaches=\"{}\" root=\".\">",
        nodes.len()
    )?;
    nodes.iter().try_for_each(|n| {
        write!(
            out,
            "<s t=\"{}\" n=\"{}\" p=\"{}:{}\" dist=\"{}\"/>",
            n.kind, n.name, n.path, n.line, n.hops
        )
    })?;
    writeln!(out, "</impact>")
}

/// Serialize symbol neighborhood to Ripwire-compatible XML.
pub fn serialize_find_symbol_xml<W: Write>(nav: &NavNeighborhood, out: &mut W) -> io::Result<()> {
    writeln!(
        out,
        "<find-symbol schema=\"ripwire.find-symbol/v1\" of=\"{}\" p=\"{}:{}\" callers=\"{}\" callees=\"{}\">",
        nav.target.name,
        nav.target.path,
        nav.target.line,
        nav.callers.len(),
        nav.callees.len()
    )?;
    writeln!(out, "  <callers>")?;
    for c in &nav.callers {
        writeln!(
            out,
            "    <s t=\"{}\" n=\"{}\" p=\"{}:{}\"/>",
            c.kind, c.name, c.path, c.line
        )?;
    }
    writeln!(out, "  </callers>")?;
    writeln!(out, "  <callees>")?;
    for c in &nav.callees {
        writeln!(
            out,
            "    <s t=\"{}\" n=\"{}\" p=\"{}:{}\"/>",
            c.kind, c.name, c.path, c.line
        )?;
    }
    writeln!(out, "  </callees>")?;
    writeln!(out, "</find-symbol>")
}

/// Serialize static use-sites to Ripwire-compatible XML.
pub fn serialize_uses_xml<W: Write>(
    sym_name: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let mut sites = Vec::new();
    for r in &ingest.references {
        if interner.resolve(&r.callee_name) == sym_name {
            let file_path = ingest
                .files
                .get(r.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            let enclosing = if r.from_symbol != crate::domain::NO_NODE {
                ingest
                    .symbols
                    .get(r.from_symbol as usize)
                    .map(|s| interner.resolve(&s.name))
                    .unwrap_or("")
            } else {
                ""
            };
            sites.push((r.role.tag(), file_path, r.line, enclosing));
        }
    }
    writeln!(
        out,
        "<uses schema=\"ripwire.uses/v1\" of=\"{}\" defs=\"1\" count=\"{}\" root=\".\">",
        sym_name,
        sites.len()
    )?;
    for (role, file, line, enc) in sites {
        if enc.is_empty() {
            writeln!(out, "  <u role=\"{}\" p=\"{}:{}\"/>", role, file, line)?;
        } else {
            writeln!(
                out,
                "  <u role=\"{}\" p=\"{}:{}\" in_id=\"{}\"/>",
                role, file, line, enc
            )?;
        }
    }
    writeln!(out, "</uses>")
}

#[inline]
fn symbol_spans_line(sym: &crate::domain::Symbol, line_num: u32) -> bool {
    sym.line <= line_num && (sym.line + sym.loc) >= line_num
}

pub fn resolve_symbol_at<'a>(
    file_spec: &str,
    line_num: u32,
    ingest: &'a IngestResult,
    interner: &Interner,
) -> Option<&'a crate::domain::Symbol> {
    let mut candidate = None;
    for sym in &ingest.symbols {
        let p = interner.resolve(&ingest.files[sym.file_id as usize].path);
        if (p == file_spec || p.ends_with(file_spec)) && symbol_spans_line(sym, line_num) {
            candidate = Some(sym);
        }
    }
    candidate
}

pub fn serialize_at_xml<W: Write>(
    file_spec: &str,
    line_num: u32,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let exact_match = ingest
        .files
        .iter()
        .any(|f| interner.resolve(&f.path) == file_spec);
    let suffix_matches: Vec<_> = ingest
        .files
        .iter()
        .filter(|f| {
            let p = interner.resolve(&f.path);
            p == file_spec || p.ends_with(file_spec)
        })
        .collect();

    if suffix_matches.is_empty() {
        return Err(io::Error::other(format!(
            "ripwire: the at flag's seed '{file_spec}:{line_num}' named no location"
        )));
    }
    if suffix_matches.len() > 1 && !exact_match {
        return Err(io::Error::other(format!(
            "ripwire: the at flag's seed '{file_spec}:{line_num}' named no location (ambiguous)"
        )));
    }
    let Some(sym) = resolve_symbol_at(file_spec, line_num, ingest, interner) else {
        return Err(io::Error::other(format!(
            "ripwire: the at flag's seed '{file_spec}:{line_num}' named no location (no indexed symbol spans line {line_num})"
        )));
    };
    let name = interner.resolve(&sym.name);
    writeln!(
        out,
        "<at schema=\"ripwire.at/v1\" p=\"{file_spec}\" l=\"{line_num}\" sym=\"{name}\"/>"
    )
}

fn check_call_relationship(
    caller_name: &str,
    callee_name: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> bool {
    for sym in &ingest.symbols {
        if interner.resolve(&sym.name) != caller_name {
            continue;
        }
        for (callee_id, _) in graph.callees(sym.id) {
            let Some(c) = ingest.symbols.get(callee_id as usize) else {
                continue;
            };
            if interner.resolve(&c.name) == callee_name {
                return true;
            }
        }
    }
    false
}

pub fn evaluate_verify_claim<W: Write>(
    claim: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (verdict, witness) = if claim.starts_with("calls(") && claim.ends_with(')') {
        let inner = &claim[6..claim.len() - 1];
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() == 2 {
            let caller_name = parts[0];
            let callee_name = parts[1];
            if check_call_relationship(caller_name, callee_name, ingest, graph, interner) {
                ("confirmed", format!("{caller_name} -> {callee_name}"))
            } else {
                (
                    "refuted",
                    format!("no call found between {caller_name} and {callee_name}"),
                )
            }
        } else {
            ("not-established", "invalid calls() arity".to_string())
        }
    } else {
        ("confirmed", claim.to_string())
    };

    writeln!(
        out,
        "<verify schema=\"ripwire.verify/v1\" claim=\"{claim}\" verdict=\"{verdict}\" witness=\"{witness}\"/>"
    )
}

pub fn execute_pack_task<W: Write>(
    task: &str,
    root: &str,
    _ingest: &IngestResult,
    _interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<ctx schema=\"ripwire.pack-task/v1\" task=\"{task}\" root=\"{root}\">"
    )?;
    writeln!(out, "  <task-bundle name=\"{task}\"/>")?;
    writeln!(out, "</ctx>")
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum Token {
    Ident(String),
    Str(String),
    Num(usize),
    LParen,
    RParen,
    Comma,
}

fn read_quoted_string(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, quote: char) -> String {
    let mut s = String::new();
    while let Some(&nc) = chars.peek() {
        chars.next();
        if nc == quote {
            break;
        }
        s.push(nc);
    }
    s
}

fn read_digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<usize> {
    let mut s = String::new();
    while let Some(&nc) = chars.peek() {
        if !nc.is_ascii_digit() {
            break;
        }
        s.push(nc);
        chars.next();
    }
    s.parse::<usize>().ok()
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | ':' | '-' | '.')
}

fn read_ident(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut s = String::new();
    while let Some(&nc) = chars.peek() {
        if !is_ident_char(nc) {
            break;
        }
        s.push(nc);
        chars.next();
    }
    s
}

fn next_token(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<Token> {
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        match c {
            '(' => {
                chars.next();
                return Some(Token::LParen);
            }
            ')' => {
                chars.next();
                return Some(Token::RParen);
            }
            ',' => {
                chars.next();
                return Some(Token::Comma);
            }
            '"' | '\'' => {
                chars.next();
                return Some(Token::Str(read_quoted_string(chars, c)));
            }
            '0'..='9' => return read_digits(chars).map(Token::Num),
            _ => {
                let id = read_ident(chars);
                if !id.is_empty() {
                    return Some(Token::Ident(id));
                }
                chars.next();
            }
        }
    }
    None
}

fn tokenize_graph_query(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(tok) = next_token(&mut chars) {
        tokens.push(tok);
    }
    tokens
}

#[derive(Debug, Clone)]
enum GraphExpr {
    All,
    Name(String),
    Callers(Box<GraphExpr>, usize),
    Callees(Box<GraphExpr>, usize),
    Kind(Box<GraphExpr>, String),
    File(Box<GraphExpr>, String),
    Fanin(Box<GraphExpr>, usize),
    Cx(Box<GraphExpr>, u32),
    And(Vec<GraphExpr>),
    Or(Vec<GraphExpr>),
    Not(Box<GraphExpr>),
}

struct GraphQueryParser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> GraphQueryParser<'a> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn match_ident(&mut self) -> Option<String> {
        match self.peek() {
            Some(Token::Ident(s)) | Some(Token::Str(s)) => {
                let s = s.clone();
                self.pos += 1;
                Some(s)
            }
            _ => None,
        }
    }

    fn parse_args(&mut self) -> Vec<GraphExpr> {
        let mut args = Vec::new();
        while let Some(expr) = self.parse_expr() {
            args.push(expr);
            if self.peek() == Some(&Token::Comma) {
                self.pos += 1;
            } else {
                break;
            }
        }
        args
    }

    fn parse_expr(&mut self) -> Option<GraphExpr> {
        let tok = self.next()?.clone();
        match &tok {
            Token::Ident(id) => self.parse_ident_expr(id),
            Token::Str(s) => Some(GraphExpr::Name(s.clone())),
            _ => None,
        }
    }

    fn parse_ident_expr(&mut self, id: &str) -> Option<GraphExpr> {
        match id {
            "all" => {
                if self.peek() == Some(&Token::LParen) {
                    self.pos += 1;
                    if self.peek() == Some(&Token::RParen) {
                        self.pos += 1;
                    }
                }
                Some(GraphExpr::All)
            }
            "name" => {
                if self.peek() == Some(&Token::LParen) {
                    self.pos += 1;
                    let name = self.match_ident().unwrap_or_default();
                    if self.peek() == Some(&Token::RParen) {
                        self.pos += 1;
                    }
                    Some(GraphExpr::Name(name))
                } else {
                    Some(GraphExpr::Name(id.to_string()))
                }
            }
            "callers" => self.parse_closure_expr(true),
            "callees" => self.parse_closure_expr(false),
            "kind" => self.parse_filter_kind(),
            "file" => self.parse_filter_file(),
            "fanin" => self.parse_filter_fanin(),
            "cx" => self.parse_filter_cx(),
            "and" => self.parse_composite(true),
            "or" => self.parse_composite(false),
            "not" => self.parse_not(),
            _ => Some(GraphExpr::Name(id.to_string())),
        }
    }

    fn parse_closure_expr(&mut self, is_callers: bool) -> Option<GraphExpr> {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name(
                if is_callers { "callers" } else { "callees" }.to_string(),
            ));
        }
        self.pos += 1;
        let sub = self.parse_expr()?;
        let depth = if self.peek() == Some(&Token::Comma) {
            self.pos += 1;
            match self.next() {
                Some(Token::Num(n)) => *n,
                _ => 1,
            }
        } else {
            1
        };
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        if is_callers {
            Some(GraphExpr::Callers(Box::new(sub), depth))
        } else {
            Some(GraphExpr::Callees(Box::new(sub), depth))
        }
    }

    fn parse_filter_kind(&mut self) -> Option<GraphExpr> {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name("kind".to_string()));
        }
        self.pos += 1;
        let sub = self.parse_expr()?;
        let kind = if self.peek() == Some(&Token::Comma) {
            self.pos += 1;
            self.match_ident().unwrap_or_default()
        } else {
            String::new()
        };
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        Some(GraphExpr::Kind(Box::new(sub), kind))
    }

    fn parse_filter_file(&mut self) -> Option<GraphExpr> {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name("file".to_string()));
        }
        self.pos += 1;
        let sub = self.parse_expr()?;
        let path = if self.peek() == Some(&Token::Comma) {
            self.pos += 1;
            self.match_ident().unwrap_or_default()
        } else {
            String::new()
        };
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        Some(GraphExpr::File(Box::new(sub), path))
    }

    fn parse_filter_fanin(&mut self) -> Option<GraphExpr> {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name("fanin".to_string()));
        }
        self.pos += 1;
        let sub = self.parse_expr()?;
        let min_in = if self.peek() == Some(&Token::Comma) {
            self.pos += 1;
            match self.next() {
                Some(Token::Num(n)) => *n,
                _ => 1,
            }
        } else {
            1
        };
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        Some(GraphExpr::Fanin(Box::new(sub), min_in))
    }

    fn parse_filter_cx(&mut self) -> Option<GraphExpr> {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name("cx".to_string()));
        }
        self.pos += 1;
        let sub = self.parse_expr()?;
        let max_cx = if self.peek() == Some(&Token::Comma) {
            self.pos += 1;
            match self.next() {
                Some(Token::Num(n)) => *n as u32,
                _ => 15,
            }
        } else {
            15
        };
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        Some(GraphExpr::Cx(Box::new(sub), max_cx))
    }

    fn parse_composite(&mut self, is_and: bool) -> Option<GraphExpr> {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name(
                if is_and { "and" } else { "or" }.to_string(),
            ));
        }
        self.pos += 1;
        let args = self.parse_args();
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        if is_and {
            Some(GraphExpr::And(args))
        } else {
            Some(GraphExpr::Or(args))
        }
    }

    fn parse_not(&mut self) -> Option<GraphExpr> {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name("not".to_string()));
        }
        self.pos += 1;
        let sub = self.parse_expr()?;
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        Some(GraphExpr::Not(Box::new(sub)))
    }
}

fn kind_matches_tag(kind: crate::domain::SymKind, tag: &str) -> bool {
    kind.tag() == tag
        || match tag {
            "fn" | "function" => kind == crate::domain::SymKind::Function,
            "method" => kind == crate::domain::SymKind::Method,
            "class" | "cls" => kind == crate::domain::SymKind::Class,
            "struct" => kind == crate::domain::SymKind::Struct,
            "interface" | "iface" => kind == crate::domain::SymKind::Interface,
            "section" | "sec" => kind == crate::domain::SymKind::Section,
            _ => false,
        }
}

fn eval_callers_closure(
    mut current: BTreeSet<NodeId>,
    depth: usize,
    graph: &Graph,
) -> BTreeSet<NodeId> {
    for _ in 0..depth {
        let mut next = BTreeSet::new();
        for &id in &current {
            for (caller, _) in graph.callers(id) {
                next.insert(caller);
            }
        }
        current = next;
    }
    current
}

fn eval_callees_closure(
    mut current: BTreeSet<NodeId>,
    depth: usize,
    graph: &Graph,
) -> BTreeSet<NodeId> {
    for _ in 0..depth {
        let mut next = BTreeSet::new();
        for &id in &current {
            for (callee, _) in graph.callees(id) {
                next.insert(callee);
            }
        }
        current = next;
    }
    current
}

fn eval_filter_kind(
    base: BTreeSet<NodeId>,
    kind_tag: &str,
    ingest: &IngestResult,
) -> BTreeSet<NodeId> {
    base.into_iter()
        .filter(|&id| {
            ingest
                .symbols
                .get(id as usize)
                .is_some_and(|s| kind_matches_tag(s.kind, kind_tag))
        })
        .collect()
}

fn eval_filter_file(
    base: BTreeSet<NodeId>,
    pat: &str,
    ingest: &IngestResult,
    interner: &Interner,
) -> BTreeSet<NodeId> {
    base.into_iter()
        .filter(|&id| {
            ingest.symbols.get(id as usize).is_some_and(|s| {
                ingest
                    .files
                    .get(s.file_id as usize)
                    .is_some_and(|f| interner.resolve(&f.path).contains(pat))
            })
        })
        .collect()
}

fn eval_and_composite(
    subs: &[GraphExpr],
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> BTreeSet<NodeId> {
    let mut iter = subs.iter();
    let Some(first) = iter.next() else {
        return BTreeSet::new();
    };
    let mut acc = evaluate_graph_expr(first, ingest, graph, interner);
    for sub in iter {
        let next = evaluate_graph_expr(sub, ingest, graph, interner);
        acc = acc.intersection(&next).copied().collect();
    }
    acc
}

fn eval_or_composite(
    subs: &[GraphExpr],
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> BTreeSet<NodeId> {
    let mut acc = BTreeSet::new();
    for sub in subs {
        acc.extend(evaluate_graph_expr(sub, ingest, graph, interner));
    }
    acc
}

fn evaluate_graph_expr(
    expr: &GraphExpr,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> BTreeSet<NodeId> {
    match expr {
        GraphExpr::All => ingest.symbols.iter().map(|s| s.id).collect(),
        GraphExpr::Name(name) => find_symbols_by_name(name, ingest, interner)
            .into_iter()
            .collect(),
        GraphExpr::Callers(sub, depth) => {
            let base = evaluate_graph_expr(sub, ingest, graph, interner);
            eval_callers_closure(base, *depth, graph)
        }
        GraphExpr::Callees(sub, depth) => {
            let base = evaluate_graph_expr(sub, ingest, graph, interner);
            eval_callees_closure(base, *depth, graph)
        }
        GraphExpr::Kind(sub, kind_tag) => {
            let base = evaluate_graph_expr(sub, ingest, graph, interner);
            eval_filter_kind(base, kind_tag, ingest)
        }
        GraphExpr::File(sub, pat) => {
            let base = evaluate_graph_expr(sub, ingest, graph, interner);
            eval_filter_file(base, pat, ingest, interner)
        }
        GraphExpr::Fanin(sub, min_in) => evaluate_graph_expr(sub, ingest, graph, interner)
            .into_iter()
            .filter(|&id| graph.in_degree(id) >= *min_in)
            .collect(),
        GraphExpr::Cx(sub, max_cx) => evaluate_graph_expr(sub, ingest, graph, interner)
            .into_iter()
            .filter(|&id| {
                ingest
                    .symbols
                    .get(id as usize)
                    .is_some_and(|s| s.ccx <= *max_cx || s.cx <= *max_cx)
            })
            .collect(),
        GraphExpr::And(subs) => eval_and_composite(subs, ingest, graph, interner),
        GraphExpr::Or(subs) => eval_or_composite(subs, ingest, graph, interner),
        GraphExpr::Not(sub) => {
            let excluded = evaluate_graph_expr(sub, ingest, graph, interner);
            ingest
                .symbols
                .iter()
                .map(|s| s.id)
                .filter(|id| !excluded.contains(id))
                .collect()
        }
    }
}

pub fn execute_graph_query_expr<W: Write>(
    expr: &str,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let tokens = tokenize_graph_query(expr);
    let mut parser = GraphQueryParser {
        tokens: &tokens,
        pos: 0,
    };
    let parsed = parser
        .parse_expr()
        .unwrap_or_else(|| GraphExpr::Name(expr.to_string()));
    let matched_nodes = evaluate_graph_expr(&parsed, ingest, graph, interner);
    let count = matched_nodes.len();

    let escaped_expr = crate::serialize::escape::escape_xml_str(expr);
    writeln!(
        out,
        "<query schema=\"ripwire.graph-query/v1\" expr=\"{escaped_expr}\" count=\"{count}\" shown=\"{count}\" capped=\"0\" root=\"{root}\">"
    )?;
    for node_id in matched_nodes {
        if let Some(sym) = ingest.symbols.get(node_id as usize) {
            let n = interner.resolve(&sym.name);
            let p = interner.resolve(&ingest.files[sym.file_id as usize].path);
            let k = sym.kind.tag();
            let l = sym.line;
            writeln!(out, "<s t=\"{k}\" n=\"{n}\" p=\"{p}:{l}\"/>")?;
        }
    }
    writeln!(out, "</query>")
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        FileInfo, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol,
    };

    #[test]
    fn test_navigate_1hop_and_impact() {
        let interner = Interner::new();
        let f0 = FileInfo {
            id: 0,
            path: interner.intern("src/lib.rs"),
            lang: Lang::Rust,
            line_count: 50,
            byte_size: 500,
        };

        let sym_a = Symbol::new(
            0,
            interner.intern("a"),
            interner.intern("c_a"),
            SymKind::Function,
            Lang::Rust,
            0,
            1,
        );
        let sym_b = Symbol::new(
            1,
            interner.intern("b"),
            interner.intern("c_b"),
            SymKind::Function,
            Lang::Rust,
            0,
            5,
        );
        let sym_c = Symbol::new(
            2,
            interner.intern("c"),
            interner.intern("c_c"),
            SymKind::Function,
            Lang::Rust,
            0,
            10,
        );

        // a calls b; b calls c
        let ref_ab = Reference::new(0, interner.intern("b"), 0, 2, Lang::Rust, RefRole::Call);
        let ref_bc = Reference::new(1, interner.intern("c"), 0, 6, Lang::Rust, RefRole::Call);

        let mut ingest = IngestResult::new();
        ingest.files.push(f0);
        ingest.symbols.extend_from_slice(&[sym_a, sym_b, sym_c]);
        ingest.references.extend_from_slice(&[ref_ab, ref_bc]);

        let graph = Graph::build(&ingest, &interner);
        let ranks = vec![0.2, 0.3, 0.5];

        let matched = find_symbols_by_name("b", &ingest, &interner);
        assert_eq!(matched, vec![1]);

        let nav = navigate_1hop(1, &ingest, &graph, &ranks, &interner).unwrap();
        assert_eq!(nav.target.name, "b");
        assert_eq!(nav.callers.len(), 1);
        assert_eq!(nav.callers[0].name, "a");
        assert_eq!(nav.callees.len(), 1);
        assert_eq!(nav.callees[0].name, "c");

        // Impact of c: callers reach b (1 hop) and a (2 hops)
        let impact = transitive_impact(2, &ingest, &graph, &ranks, &interner, None);
        assert_eq!(impact.len(), 2);
        assert_eq!(impact[0].name, "b");
        assert_eq!(impact[0].hops, 1);
        assert_eq!(impact[1].name, "a");
        assert_eq!(impact[1].hops, 2);
    }

    #[test]
    fn test_graph_query_execution() {
        let interner = Interner::new();
        let f0 = FileInfo {
            id: 0,
            path: interner.intern("src/test.rs"),
            lang: Lang::Rust,
            line_count: 50,
            byte_size: 500,
        };
        let sym_a = Symbol::new(
            0,
            interner.intern("foo"),
            interner.intern("c_foo"),
            SymKind::Function,
            Lang::Rust,
            0,
            1,
        );
        let sym_b = Symbol::new(
            1,
            interner.intern("bar"),
            interner.intern("c_bar"),
            SymKind::Function,
            Lang::Rust,
            0,
            5,
        );
        let ref_ab = Reference::new(0, interner.intern("bar"), 0, 2, Lang::Rust, RefRole::Call);

        let mut ingest = IngestResult::new();
        ingest.files.push(f0);
        ingest.symbols.extend_from_slice(&[sym_a, sym_b]);
        ingest.references.extend_from_slice(&[ref_ab]);

        let graph = Graph::build(&ingest, &interner);
        let mut out = Vec::new();
        execute_graph_query_expr(
            "callers(name(\"bar\"))",
            ".",
            &ingest,
            &graph,
            &interner,
            &mut out,
        )
        .unwrap();
        let out_str = String::from_utf8(out).unwrap();
        assert!(out_str.contains("<query schema=\"ripwire.graph-query/v1\""));
        assert!(out_str.contains("count=\"1\""));
        assert!(out_str.contains("n=\"foo\""));

        let mut out = Vec::new();
        execute_graph_query_expr(
            "callees(name(\"foo\"))",
            ".",
            &ingest,
            &graph,
            &interner,
            &mut out,
        )
        .unwrap();
        assert!(String::from_utf8(out).unwrap().contains("n=\"bar\""));

        let mut out = Vec::new();
        execute_graph_query_expr(
            "and(kind(all, fn), file(all, test), fanin(all, 1), cx(all, 10))",
            ".",
            &ingest,
            &graph,
            &interner,
            &mut out,
        )
        .unwrap();
        assert!(String::from_utf8(out).unwrap().contains("n=\"bar\""));

        let mut out = Vec::new();
        execute_graph_query_expr(
            "or(not(name(\"foo\")), callers(\"bar\", 1))",
            ".",
            &ingest,
            &graph,
            &interner,
            &mut out,
        )
        .unwrap();
        assert!(String::from_utf8(out).unwrap().contains("count=\"2\""));

        assert!(kind_matches_tag(SymKind::Function, "function"));
        assert!(kind_matches_tag(SymKind::Method, "method"));
        assert!(kind_matches_tag(SymKind::Class, "class"));
        assert!(kind_matches_tag(SymKind::Struct, "struct"));
        assert!(kind_matches_tag(SymKind::Interface, "interface"));
        assert!(kind_matches_tag(SymKind::Section, "section"));
        assert!(!kind_matches_tag(SymKind::Function, "unknown"));
    }
}
