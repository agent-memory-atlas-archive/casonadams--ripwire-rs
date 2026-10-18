use super::find_symbols_by_name;
use crate::domain::{IngestResult, Interner, NodeId};
use crate::graph::Graph;
use crate::serialize::XmlEscapeExt;
use std::collections::BTreeSet;
use std::io::{self, Write};

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

/// Predicate trait for filtering symbol sets in the `--graph-query` functional DSL.
pub trait GraphFilterPredicate: std::fmt::Debug + Send + Sync {
    fn matches(
        &self,
        sym: &crate::domain::Symbol,
        ingest: &IngestResult,
        graph: &Graph,
        interner: &Interner,
    ) -> bool;
}

#[derive(Debug, Clone)]
struct KindFilter(String);
impl GraphFilterPredicate for KindFilter {
    fn matches(
        &self,
        sym: &crate::domain::Symbol,
        _ingest: &IngestResult,
        _graph: &Graph,
        _interner: &Interner,
    ) -> bool {
        kind_matches_tag(sym.kind, &self.0)
    }
}

#[derive(Debug, Clone)]
struct FileFilter(String);
impl GraphFilterPredicate for FileFilter {
    fn matches(
        &self,
        sym: &crate::domain::Symbol,
        ingest: &IngestResult,
        _graph: &Graph,
        interner: &Interner,
    ) -> bool {
        ingest
            .files
            .get(sym.file_id as usize)
            .is_some_and(|f| interner.resolve(&f.path).contains(&self.0))
    }
}

#[derive(Debug, Clone)]
struct LayerFilter(String);
impl GraphFilterPredicate for LayerFilter {
    fn matches(
        &self,
        sym: &crate::domain::Symbol,
        ingest: &IngestResult,
        _graph: &Graph,
        interner: &Interner,
    ) -> bool {
        use crate::serialize::PathLayerExt;
        ingest
            .files
            .get(sym.file_id as usize)
            .and_then(|f| interner.resolve(&f.path).builtin_layer())
            .is_some_and(|l| l.eq_ignore_ascii_case(&self.0))
    }
}

#[derive(Debug, Clone)]
struct FaninFilter(usize);
impl GraphFilterPredicate for FaninFilter {
    fn matches(
        &self,
        sym: &crate::domain::Symbol,
        _ingest: &IngestResult,
        graph: &Graph,
        _interner: &Interner,
    ) -> bool {
        graph.in_degree(sym.id) >= self.0
    }
}

#[derive(Debug, Clone)]
struct CxFilter(u32);
impl GraphFilterPredicate for CxFilter {
    fn matches(
        &self,
        sym: &crate::domain::Symbol,
        _ingest: &IngestResult,
        _graph: &Graph,
        _interner: &Interner,
    ) -> bool {
        sym.ccx <= self.0 || sym.cx <= self.0
    }
}

#[derive(Debug, Clone)]
enum GraphExpr {
    All,
    Name(String),
    Callers(Box<GraphExpr>, usize),
    Callees(Box<GraphExpr>, usize),
    Filter(Box<GraphExpr>, std::sync::Arc<dyn GraphFilterPredicate>),
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
            "kind" => self.parse_str_filter("kind", |s| std::sync::Arc::new(KindFilter(s))),
            "file" => self.parse_str_filter("file", |s| std::sync::Arc::new(FileFilter(s))),
            "layer" => self.parse_str_filter("layer", |s| std::sync::Arc::new(LayerFilter(s))),
            "fanin" => self.parse_num_filter("fanin", 1, |n| std::sync::Arc::new(FaninFilter(n))),
            "cx" => self.parse_num_filter("cx", 15, |n| std::sync::Arc::new(CxFilter(n as u32))),
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

    fn parse_str_filter<F>(&mut self, op_name: &str, build: F) -> Option<GraphExpr>
    where
        F: FnOnce(String) -> std::sync::Arc<dyn GraphFilterPredicate>,
    {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name(op_name.to_string()));
        }
        self.pos += 1;
        let sub = self.parse_expr()?;
        let arg = if self.peek() == Some(&Token::Comma) {
            self.pos += 1;
            self.match_ident().unwrap_or_default()
        } else {
            String::new()
        };
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        Some(GraphExpr::Filter(Box::new(sub), build(arg)))
    }

    fn parse_num_filter<F>(
        &mut self,
        op_name: &str,
        default_num: usize,
        build: F,
    ) -> Option<GraphExpr>
    where
        F: FnOnce(usize) -> std::sync::Arc<dyn GraphFilterPredicate>,
    {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name(op_name.to_string()));
        }
        self.pos += 1;
        let sub = self.parse_expr()?;
        let num = if self.peek() == Some(&Token::Comma) {
            self.pos += 1;
            match self.next() {
                Some(Token::Num(n)) => *n,
                _ => default_num,
            }
        } else {
            default_num
        };
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        Some(GraphExpr::Filter(Box::new(sub), build(num)))
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

fn eval_filter_predicate(
    base: BTreeSet<NodeId>,
    pred: &dyn GraphFilterPredicate,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> BTreeSet<NodeId> {
    base.into_iter()
        .filter(|&id| {
            ingest
                .symbols
                .get(id as usize)
                .is_some_and(|sym| pred.matches(sym, ingest, graph, interner))
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
        GraphExpr::Filter(sub, pred) => {
            let base = evaluate_graph_expr(sub, ingest, graph, interner);
            eval_filter_predicate(base, pred.as_ref(), ingest, graph, interner)
        }
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

pub struct GraphQueryReport<'a> {
    pub expr: &'a str,
    pub matched_nodes: &'a BTreeSet<NodeId>,
    pub ingest: &'a IngestResult,
    pub interner: &'a Interner,
    pub pr_iters: u32,
    pub gauges: crate::serialize::GraphGauges,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl GraphQueryReport<'_> {
    fn window(&self) -> crate::serialize::PageWindow {
        let total = self.matched_nodes.len();
        let default_cap = self.limit.unwrap_or(total);
        crate::serialize::PageWindow::compute(total, default_cap, self.limit, self.offset)
    }
}

impl crate::serialize::XmlReport for GraphQueryReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "query"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire-rs.graph-query/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire-rs graph-query schema=ripwire-rs.graph-query/v1: graph-query expression over the symbol graph.",
        )
    }
    fn pr_iters(&self) -> Option<u32> {
        Some(self.pr_iters)
    }
    fn graph_gauges(&self) -> Option<crate::serialize::GraphGauges> {
        Some(self.gauges)
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let count = self.matched_nodes.len();
        let pw = self.window();
        write!(
            out,
            " expr=\"{}\" count=\"{count}\"{}",
            self.expr.xml_escaped(),
            pw.xml_attrs()
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let pw = self.window();
        for &node_id in self.matched_nodes.iter().skip(pw.begin).take(pw.shown) {
            if let Some(sym) = self.ingest.sym(node_id) {
                let n = &self.interner[sym.name];
                let p = self.ingest.sym_file_path(sym, self.interner);
                writeln!(
                    out,
                    "<s t=\"{}\" n=\"{n}\" p=\"{p}:{}\"/>",
                    sym.kind, sym.line
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GraphQueryPageOptions {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub fn execute_graph_query_expr<W: Write>(
    expr: &str,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    execute_graph_query_expr_paged(
        expr,
        root,
        ingest,
        graph,
        interner,
        GraphQueryPageOptions::default(),
        out,
    )
}

pub fn execute_graph_query_expr_paged<W: Write>(
    expr: &str,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    page: GraphQueryPageOptions,
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
    let pr_iters = graph
        .page_rank(&crate::rank::PageRankConfig::default())
        .iterations
        .max(1);
    use crate::serialize::XmlReport;
    GraphQueryReport {
        expr,
        matched_nodes: &matched_nodes,
        ingest,
        interner,
        pr_iters,
        gauges: graph.gauges(),
        limit: page.limit,
        offset: page.offset,
    }
    .write_xml_report(root, out)
}
