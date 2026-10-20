use crate::domain::{FileInfo, Interner, NO_NODE, NodeId, RefRole, Reference, Symbol};
use lasso::Spur;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CandBuffer {
    inline: [NodeId; 8],
    len: usize,
    heap: Vec<NodeId>,
}

impl CandBuffer {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            inline: [0; 8],
            len: 0,
            heap: Vec::new(),
        }
    }

    #[inline]
    pub fn push(&mut self, id: NodeId) {
        if self.heap.is_empty() && self.len < 8 {
            self.inline[self.len] = id;
            self.len += 1;
        } else {
            if self.heap.is_empty() {
                self.heap.reserve(self.len + 8);
                self.heap.extend_from_slice(&self.inline[..self.len]);
            }
            self.heap.push(id);
        }
    }

    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[NodeId] {
        if self.heap.is_empty() {
            &self.inline[..self.len]
        } else {
            &self.heap
        }
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        if self.heap.is_empty() {
            self.len == 0
        } else {
            self.heap.is_empty()
        }
    }
}

impl Default for CandBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for CandBuffer {
    type Target = [NodeId];

    #[inline]
    fn deref(&self) -> &[NodeId] {
        self.as_slice()
    }
}

impl AsRef<[NodeId]> for CandBuffer {
    #[inline]
    fn as_ref(&self) -> &[NodeId] {
        self.as_slice()
    }
}

impl<'a> IntoIterator for &'a CandBuffer {
    type Item = &'a NodeId;
    type IntoIter = std::slice::Iter<'a, NodeId>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

/// Resolved reference outcome with target nodes and per-target confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRef {
    pub from: NodeId,
    pub targets: Vec<NodeId>,
    pub conf_per_target: f32,
    pub is_ambiguous: bool,
}

/// Triage classification for a reference during call-graph resolution.
#[derive(Debug, Clone, PartialEq)]
pub enum RefOutcome {
    Ignored,
    Resolved(ResolvedRef),
    Unresolved,
    External,
    Declined,
}

/// Assign integer directory IDs based on the parent directory of each file.
#[must_use]
pub fn compute_file_dirs(files: &[FileInfo], interner: &Interner) -> Vec<u32> {
    let mut dir_map: HashMap<String, u32> = HashMap::new();
    let mut file_dirs = Vec::with_capacity(files.len());
    for f in files {
        let path = interner.resolve(&f.path);
        let dir = Path::new(path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string();
        let next_id = dir_map.len() as u32;
        let dir_id = *dir_map.entry(dir).or_insert(next_id);
        file_dirs.push(dir_id);
    }
    file_dirs
}

/// Index symbols by their interned name.
#[must_use]
pub fn index_symbols_by_name(symbols: &[Symbol]) -> HashMap<Spur, Vec<NodeId>> {
    let mut by_name: HashMap<Spur, Vec<NodeId>> = HashMap::new();
    for s in symbols {
        by_name.entry(s.name).or_default().push(s.id);
    }
    by_name
}

/// Select candidates via the precedence ladder:
/// Tier 1: Same file (conf = 1.0)
/// Tier 2: Same directory (conf = 0.5)
/// Tier 3: Unique same-language global (conf = 0.2), else dropped.
fn is_unique_global_group(cands: &[NodeId], symbols: &[Symbol]) -> bool {
    let Some(&first) = cands.first() else {
        return false;
    };
    let first_sym = &symbols[first as usize];
    let first_file = first_sym.file_id;
    let first_scope = first_sym.scope;
    cands.iter().all(|&id| {
        let sym = &symbols[id as usize];
        sym.file_id == first_file
            && (sym.scope == first_scope
                || sym.scope == Some(first_sym.name)
                || first_scope == Some(sym.name))
    })
}

pub fn select_precedence_tier(
    cands: &[NodeId],
    caller_file_id: u32,
    caller_dir_id: u32,
    symbols: &[Symbol],
    file_dirs: &[u32],
) -> Option<(Vec<NodeId>, f32)> {
    let same_file: Vec<NodeId> = cands
        .iter()
        .copied()
        .filter(|&id| symbols[id as usize].file_id == caller_file_id)
        .collect();
    if !same_file.is_empty() {
        return Some((same_file, 1.0));
    }

    let same_dir: Vec<NodeId> = cands
        .iter()
        .copied()
        .filter(|&id| file_dirs.get(symbols[id as usize].file_id as usize) == Some(&caller_dir_id))
        .collect();
    if !same_dir.is_empty() {
        return (same_dir.len() <= 4).then_some((same_dir, 0.5));
    }

    is_unique_global_group(cands, symbols).then(|| (cands.to_vec(), 0.2))
}

fn is_method_std_member_blocked<L: ResolutionLookup + ?Sized>(
    lookup: &L,
    r: &Reference,
    sym: &Symbol,
    caller_scope: Option<Spur>,
) -> bool {
    let Some(sym_sc) = sym.scope else {
        return true;
    };
    if caller_scope == Some(sym_sc) {
        return false;
    }
    !(caller_scope.is_none()
        && sym.file_id != r.file_id
        && lookup.file_mentions_scope(r.file_id, sym_sc))
}

fn is_unpinned_std_member_blocked<L: ResolutionLookup + ?Sized>(
    lookup: &L,
    r: &Reference,
    sym: &Symbol,
    caller_scope: Option<Spur>,
    callee_str: &str,
) -> bool {
    if !sym.lang.semantics().is_std_or_builtin_member(callee_str) {
        return false;
    }
    if let Some(q) = r.qualifier {
        return sym.scope != Some(q);
    }
    match sym.kind {
        crate::domain::SymKind::Field | crate::domain::SymKind::Var => true,
        crate::domain::SymKind::Method => {
            is_method_std_member_blocked(lookup, r, sym, caller_scope)
        }
        crate::domain::SymKind::Function => sym.file_id != r.file_id,
        _ => false,
    }
}

fn filter_matching_scope(cands: &CandBuffer, symbols: &[Symbol], q: Spur) -> CandBuffer {
    let mut qualified = CandBuffer::default();
    for &id in cands {
        let s = &symbols[id as usize];
        if s.scope == Some(q) || s.name == q {
            qualified.push(id);
        }
    }
    qualified
}

fn filter_in_file(cands: &CandBuffer, symbols: &[Symbol], file_id: u32) -> CandBuffer {
    let mut in_file = CandBuffer::default();
    for &id in cands {
        if symbols[id as usize].file_id == file_id {
            in_file.push(id);
        }
    }
    in_file
}

/// Index per-file referenced identifiers and qualifiers for cross-file receiver evidence.
#[must_use]
pub fn index_file_scope_mentions(
    files_len: usize,
    references: &[Reference],
) -> Vec<std::collections::HashSet<Spur>> {
    let mut sets = vec![std::collections::HashSet::new(); files_len];
    for r in references {
        if let Some(set) = sets.get_mut(r.file_id as usize) {
            set.insert(r.callee_name);
            if let Some(q) = r.qualifier {
                set.insert(q);
            }
        }
    }
    sets
}

/// Per file, the identifiers its imports bind to an indexed module file (`import x as y` binds `y`).
#[must_use]
pub fn index_module_qualifiers(
    files: &[FileInfo],
    references: &[Reference],
    interner: &Interner,
) -> Vec<HashMap<Spur, u32>> {
    let file_index: HashMap<&str, u32> = files
        .iter()
        .map(|f| (interner.resolve(&f.path), f.id))
        .collect();
    let mut maps = vec![HashMap::new(); files.len()];
    for r in references.iter().filter(|r| r.role == RefRole::Import) {
        let raw = interner.resolve(&r.callee_name);
        let semantics = r.lang.semantics();
        let Some(bound) = semantics
            .import_binding(raw)
            .and_then(|(_, bound)| interner.get(bound))
        else {
            continue;
        };
        let Some(importer) = files.get(r.file_id as usize) else {
            continue;
        };
        if let Some(target) =
            semantics.resolve_import_target(interner.resolve(&importer.path), raw, &file_index)
        {
            maps[r.file_id as usize].insert(bound, target);
        }
    }
    maps
}

fn is_non_table_read_write(r: &Reference) -> bool {
    matches!(r.role, RefRole::Read | RefRole::Write)
        && !r.lang.semantics().is_table_read_write_target()
}

fn classify_rejected_with_interner(r: &Reference, interner: &Interner) -> RefOutcome {
    if r.qualifier
        .and_then(|q| interner.try_resolve(&q))
        .is_some_and(|qs| r.lang.semantics().is_std_or_builtin_qualifier(qs))
    {
        return RefOutcome::External;
    }
    if interner
        .try_resolve(&r.callee_name)
        .is_some_and(|cs| r.lang.semantics().is_std_or_builtin_member(cs))
    {
        return RefOutcome::Declined;
    }
    RefOutcome::Unresolved
}

fn classify_rejected_candidates(r: &Reference, interner: Option<&Interner>) -> RefOutcome {
    if matches!(r.role, RefRole::Read | RefRole::Write) {
        return if r.lang.semantics().is_table_read_write_target() {
            RefOutcome::External
        } else {
            RefOutcome::Ignored
        };
    }
    interner.map_or(RefOutcome::Unresolved, |i| {
        classify_rejected_with_interner(r, i)
    })
}

/// Trait encapsulating symbol table and directory context for call resolution.
pub trait ResolutionLookup {
    fn symbols(&self) -> &[Symbol];
    fn file_dirs(&self) -> &[u32];
    fn by_name(&self) -> &HashMap<Spur, Vec<NodeId>>;
    fn interner(&self) -> Option<&Interner> {
        None
    }
    fn file_mentions_scope(&self, _file_id: u32, _scope: Spur) -> bool {
        false
    }
    fn qualifier_module_file(&self, _file_id: u32, _qualifier: Spur) -> Option<u32> {
        None
    }

    #[must_use]
    fn is_compatible_target(&self, r: &Reference, sym: &Symbol) -> bool {
        if sym.lang.semantics().is_table_read_write_target()
            && matches!(r.role, RefRole::Read | RefRole::Write)
        {
            return true;
        }
        if let Some(interner) = self.interner() {
            let callee_str = interner.try_resolve(&r.callee_name).unwrap_or("");
            let caller_scope = self
                .symbols()
                .get(r.from_symbol as usize)
                .and_then(|cs| cs.scope);
            if is_unpinned_std_member_blocked(self, r, sym, caller_scope, callee_str) {
                return false;
            }
        }
        let arity_ok = sym.kind == crate::domain::SymKind::Macro
            || !r.arg_count_known
            || r.arg_count <= sym.params
            || (sym.params == 0 && !sym.is_callable());
        sym.lang.is_compatible(r.lang) && arity_ok
    }

    /// Candidates a qualified reference can mean: those scoped by the qualifier, else those in the module
    /// file the qualifier is bound to by an import, else all of them unless the qualifier is std/builtin.
    #[must_use]
    fn narrow_by_qualifier(&self, cands: CandBuffer, r: &Reference, q: Spur) -> Option<CandBuffer> {
        let symbols = self.symbols();
        let qualified = filter_matching_scope(&cands, symbols, q);
        if !qualified.is_empty() {
            return Some(qualified);
        }
        if let Some(module_file) = self.qualifier_module_file(r.file_id, q) {
            let in_module = filter_in_file(&cands, symbols, module_file);
            if !in_module.is_empty() {
                return Some(in_module);
            }
        }
        let is_std_qual = self
            .interner()
            .and_then(|i| i.try_resolve(&q))
            .is_some_and(|qs| r.lang.semantics().is_std_or_builtin_qualifier(qs));
        (!is_std_qual).then_some(cands)
    }

    #[must_use]
    fn apply_qualifier_or_caller_scope(
        &self,
        cands: CandBuffer,
        r: &Reference,
    ) -> Option<CandBuffer> {
        let symbols = self.symbols();
        if let Some(q) = r.qualifier {
            return self.narrow_by_qualifier(cands, r, q);
        }
        if let Some(caller_sc) = symbols.get(r.from_symbol as usize).and_then(|cs| cs.scope) {
            let qualified = filter_matching_scope(&cands, symbols, caller_sc);
            if !qualified.is_empty() {
                return Some(qualified);
            }
        }
        Some(cands)
    }

    #[must_use]
    fn filter_ref_candidates(&self, all_cands: &[NodeId], r: &Reference) -> Option<CandBuffer> {
        let symbols = self.symbols();
        let mut cands = CandBuffer::default();
        for &id in all_cands {
            if self.is_compatible_target(r, &symbols[id as usize]) {
                cands.push(id);
            }
        }
        if cands.is_empty() {
            return None;
        }
        self.apply_qualifier_or_caller_scope(cands, r)
    }

    #[must_use]
    fn is_interface_scoped_method(&self, sym: &Symbol) -> bool {
        let Some(sc) = sym.scope else {
            return false;
        };
        let symbols = self.symbols();
        self.by_name().get(&sc).is_some_and(|ids| {
            ids.iter()
                .any(|&id| symbols[id as usize].kind == crate::domain::SymKind::Interface)
        })
    }

    #[must_use]
    fn select_interface_contract_tier(&self, cands: &[NodeId]) -> Option<(Vec<NodeId>, f32)> {
        let symbols = self.symbols();
        let iface_cands: Vec<NodeId> = cands
            .iter()
            .copied()
            .filter(|&id| self.is_interface_scoped_method(&symbols[id as usize]))
            .collect();
        is_unique_global_group(&iface_cands, symbols).then_some((iface_cands, 0.2))
    }

    #[must_use]
    fn is_call_like_ref(&self, r: &Reference) -> bool {
        if r.from_symbol == NO_NODE {
            return false;
        }
        match r.role {
            RefRole::Call | RefRole::Macro => true,
            RefRole::Read | RefRole::Write => {
                r.lang.semantics().is_table_read_write_target()
                    || self.by_name().contains_key(&r.callee_name)
            }
            _ => false,
        }
    }

    #[must_use]
    fn classify_ref(&self, r: &Reference) -> RefOutcome {
        if !self.is_call_like_ref(r) {
            return RefOutcome::Ignored;
        }
        let Some(all_cands) = self.by_name().get(&r.callee_name) else {
            return RefOutcome::External;
        };
        let Some(cands) = self.filter_ref_candidates(all_cands, r) else {
            return classify_rejected_candidates(r, self.interner());
        };
        let caller_file_id = r.file_id;
        let caller_dir_id = self
            .file_dirs()
            .get(caller_file_id as usize)
            .copied()
            .unwrap_or(u32::MAX);
        let Some((tier, conf)) = select_precedence_tier(
            &cands,
            caller_file_id,
            caller_dir_id,
            self.symbols(),
            self.file_dirs(),
        )
        .or_else(|| self.select_interface_contract_tier(&cands)) else {
            return if is_non_table_read_write(r) {
                RefOutcome::Ignored
            } else {
                RefOutcome::Declined
            };
        };
        let real_targets: Vec<NodeId> = tier
            .into_iter()
            .filter(|&target| target != r.from_symbol)
            .collect();
        if real_targets.is_empty() {
            return RefOutcome::Ignored;
        }
        let is_ambiguous = real_targets.len() > 1;
        let conf_per_target = conf / (real_targets.len() as f32);
        RefOutcome::Resolved(ResolvedRef {
            from: r.from_symbol,
            targets: real_targets,
            conf_per_target,
            is_ambiguous,
        })
    }
}

/// Concrete lookup context bundling symbol tables, directory mapping, and optional interner.
pub struct ResolutionContext<'a> {
    pub symbols: &'a [Symbol],
    pub file_dirs: &'a [u32],
    pub by_name: &'a HashMap<Spur, Vec<NodeId>>,
    pub interner: Option<&'a Interner>,
    pub file_scopes: Option<&'a [std::collections::HashSet<Spur>]>,
    pub module_qualifiers: Option<&'a [HashMap<Spur, u32>]>,
}

impl ResolutionLookup for ResolutionContext<'_> {
    #[inline]
    fn symbols(&self) -> &[Symbol] {
        self.symbols
    }

    #[inline]
    fn file_dirs(&self) -> &[u32] {
        self.file_dirs
    }

    #[inline]
    fn by_name(&self) -> &HashMap<Spur, Vec<NodeId>> {
        self.by_name
    }

    #[inline]
    fn interner(&self) -> Option<&Interner> {
        self.interner
    }

    #[inline]
    fn file_mentions_scope(&self, file_id: u32, scope: Spur) -> bool {
        self.file_scopes
            .and_then(|sets| sets.get(file_id as usize))
            .is_some_and(|set| set.contains(&scope))
    }

    #[inline]
    fn qualifier_module_file(&self, file_id: u32, qualifier: Spur) -> Option<u32> {
        self.module_qualifiers?
            .get(file_id as usize)?
            .get(&qualifier)
            .copied()
    }
}
