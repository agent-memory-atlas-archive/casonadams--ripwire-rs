use ripwire_rs::{
    Interner, Lang, LanguageRegistry, ingest_in_memory_files, resolve_all_references,
};

fn callees_of(caller: &str, files: &[(&str, Lang, &[u8])]) -> Vec<String> {
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(files, &registry, &interner);
    let resolution = resolve_all_references(&ingest, &interner);
    let from = ingest
        .symbols
        .iter()
        .find(|symbol| interner.resolve(&symbol.name) == caller)
        .unwrap()
        .id;
    let mut names: Vec<_> = resolution
        .edge_accumulators
        .keys()
        .filter(|(f, _)| *f == from)
        .map(|(_, to)| {
            interner
                .resolve(&ingest.symbols[*to as usize].name)
                .to_owned()
        })
        .collect();
    names.sort();
    names
}

const DEFINER: &[u8] = b"export function reject(code: string) { return code; }\n";

#[test]
fn imported_function_named_like_a_std_member_resolves_across_files() {
    let files: [(&str, Lang, &[u8]); 2] = [
        ("src/errors.ts", Lang::TypeScript, DEFINER),
        (
            "src/check.ts",
            Lang::TypeScript,
            b"import { reject } from \"./errors\";\nexport function check() { reject(\"bad\"); }\n",
        ),
    ];
    assert_eq!(callees_of("check", &files), ["reject"]);
}

#[test]
fn unimported_function_named_like_a_std_member_stays_declined() {
    let files: [(&str, Lang, &[u8]); 2] = [
        ("src/errors.ts", Lang::TypeScript, DEFINER),
        (
            "src/other.ts",
            Lang::TypeScript,
            b"export function check() { reject(\"bad\"); }\n",
        ),
    ];
    assert!(callees_of("check", &files).is_empty());
}
