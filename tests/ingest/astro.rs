use ripwire_rs::ingest::astro::{AstroFrontmatter, extract_astro_frontmatter};

#[test]
fn test_astro_valid_frontmatter() {
    let src = b"---\nimport Header from './Header.astro';\nconst x = 1;\n---\n<div>{x}</div>\n";
    let res = extract_astro_frontmatter(src);
    assert!(matches!(
        res,
        AstroFrontmatter::Ok {
            start_byte: 4,
            end_byte: 54,
            start_row: 1
        }
    ));
}

#[test]
fn test_astro_template_only() {
    let src = b"<div>Hello World</div>\n";
    assert_eq!(extract_astro_frontmatter(src), AstroFrontmatter::None);
}

#[test]
fn test_astro_unterminated() {
    let src = b"---\nimport Header from './Header.astro';\n";
    assert_eq!(
        extract_astro_frontmatter(src),
        AstroFrontmatter::Unterminated
    );
}
