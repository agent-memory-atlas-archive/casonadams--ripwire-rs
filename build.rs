use std::path::PathBuf;

fn compile_grammar(name: &str, files: &[&str], extra_includes: &[&str]) {
    let dir = PathBuf::from("vendor/grammars").join(name);
    let src_dir = if name == "php" {
        dir.join("php/src")
    } else {
        dir.join("src")
    };
    let mut build = cc::Build::new();
    build.include(&src_dir);
    for inc in extra_includes {
        build.include(dir.join(inc));
    }
    for file in files {
        let path = src_dir.join(file);
        println!("cargo:rerun-if-changed={}", path.display());
        build.file(path);
    }
    build.warnings(false);
    build.flag_if_supported("/std:c11");
    build.flag_if_supported("-std=c11");
    build.compile(&format!("tree_sitter_{name}"));
}

fn main() {
    assert!(
        PathBuf::from("vendor/grammars").exists(),
        "Missing 'vendor/grammars' directory. Run 'make update-grammars' or './scripts/update-grammars.sh' before building."
    );

    let grammars: &[(&str, &[&str], &[&str])] = &[
        ("elixir", &["parser.c", "scanner.c"], &[]),
        ("dart", &["parser.c", "scanner.c"], &[]),
        ("kotlin", &["parser.c", "scanner.c"], &[]),
        ("ruby", &["parser.c", "scanner.c"], &[]),
        ("php", &["parser.c", "scanner.c"], &["common"]),
        ("lua", &["parser.c", "scanner.c"], &[]),
        ("gdscript", &["parser.c", "scanner.c"], &[]),
        ("json", &["parser.c"], &[]),
        ("toml", &["parser.c", "scanner.c"], &[]),
        ("yaml", &["parser.c", "scanner.c", "schema.core.c"], &[]),
        ("markdown", &["parser.c", "scanner.c"], &[]),
    ];

    for (name, files, extra_includes) in grammars {
        compile_grammar(name, files, extra_includes);
    }
}
