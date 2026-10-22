//! Value reach: a symbol reaches the functions and statics it names as values, without calling them.
//!
//! A registry table holds handlers as values (`handler: call_map`), and a `static` initializer runs
//! the code of the functions it calls, so a function that reads that static executes that code.
//! Only same-file names count, so the reading stays local. Call edges are not changed: this is a
//! second, reach-only adjacency for the test-reach gates.

use crate::domain::{IngestResult, NO_NODE, NodeId, RefRole, SymKind};
use lasso::Spur;
use std::collections::HashMap;

/// For each symbol, the symbols it reaches by value (`forward`) and the ones that reach it (`reverse`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ValueReach {
    forward: Vec<Vec<NodeId>>,
    reverse: Vec<Vec<NodeId>>,
}

impl ValueReach {
    #[must_use]
    pub fn build(ingest: &IngestResult) -> Self {
        let n = ingest.symbols.len();
        let mut by_name: HashMap<(u32, Spur), Vec<NodeId>> = HashMap::new();
        for s in &ingest.symbols {
            if matches!(s.kind, SymKind::Function | SymKind::Method | SymKind::Var) {
                by_name.entry((s.file_id, s.name)).or_default().push(s.id);
            }
        }
        let mut forward = vec![Vec::new(); n];
        let mut reverse = vec![Vec::new(); n];
        for r in &ingest.references {
            if r.role != RefRole::Read || r.from_symbol == NO_NODE {
                continue;
            }
            let Some(targets) = by_name.get(&(r.file_id, r.callee_name)) else {
                continue;
            };
            for &target in targets {
                if target != r.from_symbol {
                    forward[r.from_symbol as usize].push(target);
                    reverse[target as usize].push(r.from_symbol);
                }
            }
        }
        for list in forward.iter_mut().chain(reverse.iter_mut()) {
            list.sort_unstable();
            list.dedup();
        }
        Self { forward, reverse }
    }

    /// Symbols `node` reaches by value.
    pub fn targets(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.forward
            .get(node as usize)
            .into_iter()
            .flatten()
            .copied()
    }

    /// Symbols that reach `node` by value.
    pub fn readers(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.reverse
            .get(node as usize)
            .into_iter()
            .flatten()
            .copied()
    }
}
