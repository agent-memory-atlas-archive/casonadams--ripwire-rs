use super::TraitEnv;
use crate::domain::{FileInfo, Lang, NodeId, SymKind, Symbol};
use lasso::Spur;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct Scope {
    pub file_id: u32,
    pub module: Option<NodeId>,
}

impl Scope {
    fn root(file_id: u32) -> Self {
        Self {
            file_id,
            module: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct RustScopes {
    by_file: HashMap<u32, Vec<NodeId>>,
    parents: HashMap<NodeId, Scope>,
    children: HashMap<(Scope, Spur), Vec<NodeId>>,
    external: HashMap<NodeId, u32>,
    file_parents: HashMap<u32, Option<Scope>>,
}

impl RustScopes {
    pub fn build(files: &[FileInfo], env: &TraitEnv<'_>) -> Self {
        let mut index = Self::default();
        for symbol in env
            .symbols
            .iter()
            .filter(|s| s.lang == Lang::Rust && s.kind == SymKind::Section)
        {
            index
                .by_file
                .entry(symbol.file_id)
                .or_default()
                .push(symbol.id);
        }
        index.index_parents(env.symbols);
        index.index_external(files, env);
        index
    }

    fn index_parents(&mut self, symbols: &[Symbol]) {
        let mut stack: Vec<NodeId> = Vec::new();
        for (&file_id, modules) in &mut self.by_file {
            modules.sort_unstable_by_key(|&id| (symbols[id as usize].sig_start_byte, id));
            stack.clear();
            for &id in modules.iter() {
                let start = symbols[id as usize].sig_start_byte;
                while stack
                    .last()
                    .is_some_and(|&parent| symbols[parent as usize].end_byte <= start)
                {
                    stack.pop();
                }
                let parent = Scope {
                    file_id,
                    module: stack.last().copied(),
                };
                self.parents.insert(id, parent);
                self.children
                    .entry((parent, symbols[id as usize].name))
                    .or_default()
                    .push(id);
                stack.push(id);
            }
        }
    }

    fn index_external(&mut self, files: &[FileInfo], env: &TraitEnv<'_>) {
        let file_index: HashMap<&str, u32> = files
            .iter()
            .map(|file| (env.interner.resolve(&file.path), file.id))
            .collect();
        let mut lineage = Vec::new();
        let mut path = String::new();
        for &id in self.parents.keys() {
            let symbol = &env.symbols[id as usize];
            if symbol.has_body() {
                continue;
            }
            lineage.clear();
            let mut module = Some(id);
            while let Some(id) = module {
                lineage.push(id);
                module = self.parents[&id].module;
            }
            path.clear();
            path.push_str("self");
            for &id in lineage.iter().rev() {
                path.push_str("::");
                path.push_str(env.interner.resolve(&env.symbols[id as usize].name));
            }
            let Some(file) = files.get(symbol.file_id as usize) else {
                continue;
            };
            let Some(target) = Lang::Rust.semantics().resolve_import_target(
                env.interner.resolve(&file.path),
                &path,
                &file_index,
            ) else {
                continue;
            };
            self.external.insert(id, target);
            let child_scope = Scope {
                file_id: symbol.file_id,
                module: Some(id),
            };
            self.file_parents
                .entry(target)
                .and_modify(|existing| {
                    if *existing != Some(child_scope) {
                        *existing = None;
                    }
                })
                .or_insert(Some(child_scope));
        }
    }

    pub fn at(&self, file_id: u32, byte: u32, symbols: &[Symbol]) -> Scope {
        let root = Scope::root(file_id);
        let Some(modules) = self.by_file.get(&file_id) else {
            return root;
        };
        let before = modules.partition_point(|&id| symbols[id as usize].sig_start_byte <= byte);
        let Some(&id) = before.checked_sub(1).and_then(|i| modules.get(i)) else {
            return root;
        };
        let mut scope = Scope {
            file_id,
            module: Some(id),
        };
        while let Some(id) = scope.module {
            if byte < symbols[id as usize].end_byte {
                break;
            }
            scope = self.parents[&id];
        }
        scope
    }

    pub fn parent(&self, scope: Scope) -> Option<Scope> {
        match scope.module {
            Some(id) => self.parents.get(&id).copied(),
            None => self.file_parents.get(&scope.file_id).copied().flatten(),
        }
    }

    fn child(&self, scope: Scope, name: &str, env: &TraitEnv<'_>) -> Option<Scope> {
        let name = env.interner.get(name)?;
        let [id] = self.children.get(&(scope, name))?.as_slice() else {
            return None;
        };
        if env.symbols[*id as usize].has_body() {
            return Some(Scope {
                file_id: scope.file_id,
                module: Some(*id),
            });
        }
        self.external.get(id).copied().map(Scope::root)
    }

    pub fn qualify(
        &self,
        path: &str,
        site: Scope,
        root: Option<u32>,
        env: &TraitEnv<'_>,
    ) -> Option<Scope> {
        let mut parts = path.split("::");
        let first = parts.next()?;
        let mut scope = match first {
            "crate" => Scope::root(root?),
            "self" => site,
            "super" => self.parent(site)?,
            _ => self.child(site, first, env)?,
        };
        for part in parts {
            scope = if part == "super" {
                self.parent(scope)?
            } else {
                self.child(scope, part, env)?
            };
        }
        Some(scope)
    }
}
