use rwire::{Interner, Lang, NodeId, SymKind, Symbol, NO_NODE};

fn main() {
    let interner = Interner::new();
    let sym_name = interner.intern("main");
    let canon_id = interner.intern("src/main.rs::main");

    let sym = Symbol::new(
        0 as NodeId,
        sym_name,
        canon_id,
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    );

    println!(
        "rwire v0.1.0 (initialized) — symbol '{}' [{}] at file:{} line:{}",
        interner.resolve(&sym.name),
        sym.kind.tag(),
        sym.file_id,
        sym.line
    );
    assert_ne!(sym.id, NO_NODE);
}
