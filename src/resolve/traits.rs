use super::scopes::{RustScopes, Scope};
use crate::domain::{FileInfo, Implementation, Interner, NodeId, SymKind, Symbol};
use lasso::Spur;
use std::collections::{HashMap, HashSet};

/// Symbol, directory, interner, and module-qualifier lookups that owner resolution reads.
pub struct TraitEnv<'a> {
    pub symbols: &'a [Symbol],
    pub file_dirs: &'a [u32],
    pub interner: &'a Interner,
    pub qualifier_file: &'a dyn Fn(u32, Spur) -> Option<u32>,
}

/// One impl block. `trait_id` is set only when the trait is an indexed local trait; `subject`
/// is the concrete type the block implements for; `bound` is the trait a blanket impl requires.
#[derive(Debug, Clone)]
pub struct TraitImpl {
    pub trait_id: Option<NodeId>,
    pub subject: Option<NodeId>,
    pub bound: Option<NodeId>,
    pub methods: Vec<NodeId>,
}

#[derive(Debug, Default, Clone)]
pub struct TraitIndex {
    impls: Vec<TraitImpl>,
    by_trait: HashMap<NodeId, Vec<usize>>,
    by_subject: HashMap<NodeId, Vec<usize>>,
    blankets: Vec<usize>,
    trait_defaults: HashMap<NodeId, Vec<NodeId>>,
    nominals_by_name: HashMap<Spur, Vec<NodeId>>,
    scopes: RustScopes,
    crate_roots: Vec<Option<u32>>,
    non_inherent: HashSet<NodeId>,
}

type MethodsByFile = HashMap<u32, Vec<NodeId>>;

/// Which nominal kinds a path may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Want {
    Any,
    Trait,
    Concrete,
}

impl Want {
    fn admits(self, kind: SymKind) -> bool {
        match self {
            Self::Any => true,
            Self::Trait => kind == SymKind::Interface,
            Self::Concrete => kind != SymKind::Interface,
        }
    }
}

impl TraitIndex {
    #[must_use]
    pub fn build(
        implementations: &[Implementation],
        files: &[FileInfo],
        env: &TraitEnv<'_>,
    ) -> Self {
        let mut index = Self {
            nominals_by_name: nominals_by_name(env.symbols),
            scopes: RustScopes::build(files, env),
            crate_roots: crate_roots(files, env.interner),
            ..Self::default()
        };
        let methods = methods_by_file(env.symbols);
        index.index_trait_bodies(env, &methods);
        for imp in implementations {
            index.add_implementation(imp, env, &methods);
        }
        index
    }

    /// Methods declared in a trait body or a trait impl block: never inherent methods of a type.
    #[must_use]
    pub fn is_non_inherent(&self, method: NodeId) -> bool {
        self.non_inherent.contains(&method)
    }

    /// The type a receiver names (concrete or trait) as seen from `file_id`, or `None` when
    /// the name cannot be proven to denote one indexed type.
    #[must_use]
    pub fn resolve_receiver_owner(
        &self,
        receiver: &str,
        file_id: u32,
        byte: u32,
        env: &TraitEnv<'_>,
    ) -> Option<NodeId> {
        self.resolve_path(receiver, file_id, byte, env, Want::Any)
    }

    /// Concrete subject types with an impl of `trait_id`.
    #[must_use]
    pub fn implementor_ids(&self, trait_id: NodeId) -> Vec<NodeId> {
        let mut out: Vec<NodeId> = self
            .by_trait
            .get(&trait_id)
            .into_iter()
            .flatten()
            .filter_map(|&i| self.impls[i].subject)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Methods named `name` a call through trait `trait_id` may reach: each impl's override, or
    /// the trait default where the impl does not override.
    #[must_use]
    pub fn dispatch(&self, trait_id: NodeId, name: Spur, symbols: &[Symbol]) -> Vec<NodeId> {
        let idxs = self
            .by_trait
            .get(&trait_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut out = Vec::new();
        if idxs.is_empty() {
            out.extend(self.default_methods(trait_id, name, symbols));
        }
        for &i in idxs {
            out.extend(self.impl_targets(&self.impls[i], name, symbols));
        }
        sort_dedup(out)
    }

    /// Methods named `name` available on a concrete subject: its impls, plus blanket impls
    /// whose bound the subject implements.
    #[must_use]
    pub fn subject_targets(&self, subject: NodeId, name: Spur, symbols: &[Symbol]) -> Vec<NodeId> {
        let mut out = Vec::new();
        for &i in self
            .by_subject
            .get(&subject)
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            out.extend(self.impl_targets(&self.impls[i], name, symbols));
        }
        for &i in &self.blankets {
            let imp = &self.impls[i];
            if imp
                .bound
                .is_some_and(|bound| self.implements(subject, bound))
            {
                out.extend(self.impl_targets(imp, name, symbols));
            }
        }
        sort_dedup(out)
    }

    fn implements(&self, subject: NodeId, trait_id: NodeId) -> bool {
        self.by_subject.get(&subject).is_some_and(|idxs| {
            idxs.iter()
                .any(|&i| self.impls[i].trait_id == Some(trait_id))
        })
    }

    fn impl_targets(&self, imp: &TraitImpl, name: Spur, symbols: &[Symbol]) -> Vec<NodeId> {
        let own: Vec<NodeId> = imp
            .methods
            .iter()
            .copied()
            .filter(|&m| named_with_body(symbols, m, name))
            .collect();
        match (own.is_empty(), imp.trait_id) {
            (true, Some(trait_id)) => self.default_methods(trait_id, name, symbols),
            _ => own,
        }
    }

    fn default_methods(&self, trait_id: NodeId, name: Spur, symbols: &[Symbol]) -> Vec<NodeId> {
        self.trait_defaults
            .get(&trait_id)
            .map(|ids| {
                ids.iter()
                    .copied()
                    .filter(|&m| symbols[m as usize].name == name)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn index_trait_bodies(&mut self, env: &TraitEnv<'_>, methods: &MethodsByFile) {
        let traits: Vec<NodeId> = self
            .nominals_by_name
            .values()
            .flatten()
            .copied()
            .filter(|&id| env.symbols[id as usize].kind == SymKind::Interface)
            .collect();
        for trait_id in traits {
            let t = &env.symbols[trait_id as usize];
            let members = span_methods(
                env.symbols,
                file_methods(methods, t.file_id),
                t.sig_start_byte,
                t.end_byte,
            );
            for &m in members {
                self.non_inherent.insert(m);
                if env.symbols[m as usize].has_body() {
                    self.trait_defaults.entry(trait_id).or_default().push(m);
                }
            }
        }
    }

    fn add_implementation(
        &mut self,
        imp: &Implementation,
        env: &TraitEnv<'_>,
        methods: &MethodsByFile,
    ) {
        let members = span_methods(
            env.symbols,
            file_methods(methods, imp.file_id),
            imp.start_byte,
            imp.end_byte,
        );
        if imp.trait_name.is_some() {
            self.non_inherent.extend(members.iter().copied());
        }
        let trait_id = self.resolve_trait_of(imp, env);
        let (subject, bound) = if imp.generic_subject {
            (
                None,
                imp.subject_bound.and_then(|b| {
                    self.resolve_text(b, imp.file_id, imp.start_byte, env, Want::Trait)
                }),
            )
        } else {
            (
                self.resolve_text(
                    imp.subject,
                    imp.file_id,
                    imp.start_byte,
                    env,
                    Want::Concrete,
                ),
                None,
            )
        };
        if trait_id.is_none() && subject.is_none() && bound.is_none() {
            return;
        }
        let idx = self.impls.len();
        self.impls.push(TraitImpl {
            trait_id,
            subject,
            bound,
            methods: members.to_vec(),
        });
        if let Some(trait_id) = trait_id {
            self.by_trait.entry(trait_id).or_default().push(idx);
        }
        if imp.generic_subject {
            if bound.is_some() {
                self.blankets.push(idx);
            }
        } else if let Some(subject) = subject {
            self.by_subject.entry(subject).or_default().push(idx);
        }
    }

    fn resolve_trait_of(&self, imp: &Implementation, env: &TraitEnv<'_>) -> Option<NodeId> {
        let name = env.interner.try_resolve(&imp.trait_name?)?;
        let text = match imp.trait_qualifier {
            Some(q) => format!("{}::{name}", env.interner.try_resolve(&q)?),
            None => name.to_owned(),
        };
        self.resolve_path(&text, imp.file_id, imp.start_byte, env, Want::Trait)
    }

    fn resolve_text(
        &self,
        text: Spur,
        file_id: u32,
        byte: u32,
        env: &TraitEnv<'_>,
        want: Want,
    ) -> Option<NodeId> {
        self.resolve_path(env.interner.try_resolve(&text)?, file_id, byte, env, want)
    }

    fn resolve_path(
        &self,
        text: &str,
        file_id: u32,
        byte: u32,
        env: &TraitEnv<'_>,
        want: Want,
    ) -> Option<NodeId> {
        let (quals, leaf) = text.rsplit_once("::").unwrap_or(("", text));
        let leaf = env.interner.get(leaf)?;
        let symbols = env.symbols;
        let admitted = self
            .nominals_by_name
            .get(&leaf)?
            .iter()
            .copied()
            .filter(|&id| want.admits(symbols[id as usize].kind));
        let site = self.scopes.at(file_id, byte, symbols);
        if !quals.is_empty() {
            let scope = self
                .scopes
                .qualify(quals, site, self.crate_roots[file_id as usize], env)
                .or_else(|| self.qualified_file(quals, file_id, env))?;
            return only(admitted.filter(|&id| {
                self.scopes.at(
                    symbols[id as usize].file_id,
                    symbols[id as usize].sig_start_byte,
                    symbols,
                ) == scope
            }));
        }
        let admitted: Vec<NodeId> = admitted.collect();
        let mut scope = Some(site);
        while let Some(current) = scope {
            let found = only_or_ambiguous(admitted.iter().copied().filter(|&id| {
                let symbol = &symbols[id as usize];
                self.scopes
                    .at(symbol.file_id, symbol.sig_start_byte, symbols)
                    == current
            }));
            match found {
                Match::One(id) => return Some(id),
                Match::Ambiguous => return None,
                Match::None => scope = self.scopes.parent(current),
            }
        }
        self.visible_unique(&admitted, file_id, env)
    }

    fn qualified_file(&self, qualifier: &str, file_id: u32, env: &TraitEnv<'_>) -> Option<Scope> {
        let qualifier = env.interner.get(qualifier)?;
        let target = (env.qualifier_file)(file_id, qualifier)?;
        Some(Scope {
            file_id: target,
            module: None,
        })
    }

    fn visible_unique(
        &self,
        admitted: &[NodeId],
        file_id: u32,
        env: &TraitEnv<'_>,
    ) -> Option<NodeId> {
        let symbols = env.symbols;
        let in_file = admitted
            .iter()
            .copied()
            .filter(|&id| symbols[id as usize].file_id == file_id);
        match only_or_ambiguous(in_file) {
            Match::One(id) => return Some(id),
            Match::Ambiguous => return None,
            Match::None => {}
        }
        let dir = env.file_dirs.get(file_id as usize).copied()?;
        let in_dir = admitted.iter().copied().filter(|&id| {
            env.file_dirs
                .get(symbols[id as usize].file_id as usize)
                .copied()
                == Some(dir)
        });
        match only_or_ambiguous(in_dir) {
            Match::One(id) => Some(id),
            Match::Ambiguous => None,
            Match::None => only(admitted.iter().copied()),
        }
    }
}

fn nominals_by_name(symbols: &[Symbol]) -> HashMap<Spur, Vec<NodeId>> {
    let mut map: HashMap<Spur, Vec<NodeId>> = HashMap::new();
    for s in symbols
        .iter()
        .filter(|s| s.lang.semantics().has_trait_impls() && s.kind.is_type_container())
    {
        map.entry(s.name).or_default().push(s.id);
    }
    map
}

fn methods_by_file(symbols: &[Symbol]) -> MethodsByFile {
    let mut map: MethodsByFile = HashMap::new();
    for s in symbols
        .iter()
        .filter(|s| s.kind == SymKind::Method && s.lang.semantics().has_trait_impls())
    {
        map.entry(s.file_id).or_default().push(s.id);
    }
    for ids in map.values_mut() {
        ids.sort_unstable_by_key(|&id| symbols[id as usize].sig_start_byte);
    }
    map
}

fn file_methods(methods: &MethodsByFile, file_id: u32) -> &[NodeId] {
    methods.get(&file_id).map(Vec::as_slice).unwrap_or(&[])
}

fn span_methods<'a>(
    symbols: &[Symbol],
    methods: &'a [NodeId],
    start: u32,
    end: u32,
) -> &'a [NodeId] {
    let lo = methods.partition_point(|&id| symbols[id as usize].sig_start_byte < start);
    let hi = methods.partition_point(|&id| symbols[id as usize].sig_start_byte < end);
    &methods[lo..hi.max(lo)]
}

fn named_with_body(symbols: &[Symbol], id: NodeId, name: Spur) -> bool {
    let s = &symbols[id as usize];
    s.name == name && s.has_body()
}

fn sort_dedup(mut ids: Vec<NodeId>) -> Vec<NodeId> {
    ids.sort_unstable();
    ids.dedup();
    ids
}

enum Match {
    None,
    One(NodeId),
    Ambiguous,
}

fn only_or_ambiguous(ids: impl IntoIterator<Item = NodeId>) -> Match {
    let mut it = ids.into_iter();
    match (it.next(), it.next()) {
        (None, _) => Match::None,
        (Some(id), None) => Match::One(id),
        _ => Match::Ambiguous,
    }
}

fn only(ids: impl IntoIterator<Item = NodeId>) -> Option<NodeId> {
    let mut it = ids.into_iter();
    let first = it.next()?;
    it.next().is_none().then_some(first)
}

/// The crate root file of every file: the unique `lib.rs`/`main.rs` under the deepest `src/`
/// directory containing it. A root file is its own crate root.
fn crate_roots(files: &[FileInfo], interner: &Interner) -> Vec<Option<u32>> {
    let paths: Vec<(u32, &str)> = files
        .iter()
        .filter_map(|f| Some((f.id, interner.try_resolve(&f.path)?)))
        .collect();
    let roots: Vec<(u32, &str)> = files
        .iter()
        .filter_map(|f| {
            let path = interner.try_resolve(&f.path)?;
            let suffixes = f.lang.semantics().crate_root_suffixes();
            suffixes
                .iter()
                .any(|s| path.ends_with(s))
                .then_some((f.id, path))
        })
        .collect();
    let len = files.iter().map(|f| f.id as usize + 1).max().unwrap_or(0);
    let mut out = vec![None; len];
    for &(id, path) in &paths {
        out[id as usize] = crate_root_of(id, path, &roots);
    }
    out
}

fn crate_root_of(id: u32, path: &str, roots: &[(u32, &str)]) -> Option<u32> {
    if roots.iter().any(|&(root, _)| root == id) {
        return Some(id);
    }
    let containing: Vec<(usize, u32)> = roots
        .iter()
        .filter_map(|&(root, root_path)| {
            let dir = &root_path[..=root_path.rfind('/')?];
            path.starts_with(dir).then_some((dir.len(), root))
        })
        .collect();
    let deepest = containing.iter().map(|&(depth, _)| depth).max()?;
    only(
        containing
            .into_iter()
            .filter(|&(depth, _)| depth == deepest)
            .map(|(_, root)| root),
    )
}
