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
    install_man_pages();
    embed_skills();
}

fn copy_man_to_dir(src: &std::path::Path, dir: PathBuf) {
    if dir.exists() || std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::copy(src.join("rw.1"), dir.join("rw.1"));
        let _ = std::fs::copy(src.join("ripwire.1"), dir.join("ripwire.1"));
    }
}

fn install_man_pages() {
    println!("cargo:rerun-if-changed=man/man1/rw.1");
    println!("cargo:rerun-if-changed=man/man1/ripwire.1");

    let src = PathBuf::from("man/man1");
    if !src.join("rw.1").exists() {
        return;
    }

    if let Ok(home) = std::env::var("HOME") {
        copy_man_to_dir(&src, PathBuf::from(home).join(".local/share/man/man1"));
    }
    copy_man_to_dir(&src, PathBuf::from("/opt/homebrew/share/man/man1"));
}
fn collect_skills(
    base: &std::path::Path,
    dir: &std::path::Path,
    entries: &mut Vec<(String, String)>,
) {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_skills(base, &path, entries);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name == "CONSOLIDATION.md" || file_name == "README.md" {
                continue;
            }
            let Ok(stripped) = path.strip_prefix(base) else {
                continue;
            };
            let Some(rel_src) = stripped.to_str() else {
                continue;
            };
            let rel_dest = rel_src.strip_prefix("hermes/").unwrap_or(rel_src);
            entries.push((rel_dest.to_string(), rel_src.to_string()));
        }
    }
}

fn embed_skills() {
    println!("cargo:rerun-if-changed=skills");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_else(|_| "target".into()));
    let dest_path = out_dir.join("embedded_skills.rs");

    let skills_dir = PathBuf::from("skills");
    if !skills_dir.exists() {
        let _ = std::fs::write(
            &dest_path,
            "pub static EMBEDDED_SKILLS: &[(&str, &str)] = &[];\n",
        );
        return;
    }

    let mut entries = Vec::new();
    collect_skills(&skills_dir, &skills_dir, &mut entries);
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut code = String::from("pub static EMBEDDED_SKILLS: &[(&str, &str)] = &[\n");
    for (rel_dest, rel_src) in entries {
        code.push_str(&format!(
            "    ({:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/skills/{}\"))),\n",
            rel_dest, rel_src
        ));
    }
    code.push_str("];\n");

    let _ = std::fs::write(&dest_path, code);
}
