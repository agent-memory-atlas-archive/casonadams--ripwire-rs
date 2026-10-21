use crate::domain::Lang;
use crate::languages::source::{SourceRegion, source_regions, source_regions_in_tree};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskedSource {
    pub code: String,
    pub comments: String,
}

fn masked_view(bytes: &[u8], regions: &[SourceRegion], keep: SourceRegion) -> String {
    let view: Vec<u8> = bytes
        .iter()
        .zip(regions)
        .map(|(&b, &r)| match b {
            b'\n' => b'\n',
            _ if r == keep => b,
            _ => b' ',
        })
        .collect();
    String::from_utf8(view).expect("source spans preserve UTF-8")
}

#[must_use]
pub fn mask_source(src: &str, lang: Lang) -> MaskedSource {
    mask_regions(src, &source_regions(src, lang, ""))
}

#[must_use]
pub fn mask_source_in_tree(src: &str, lang: Lang, tree: &tree_sitter::Tree) -> MaskedSource {
    mask_regions(src, &source_regions_in_tree(tree, src.len(), lang))
}

fn mask_regions(src: &str, regions: &[SourceRegion]) -> MaskedSource {
    MaskedSource {
        code: masked_view(src.as_bytes(), regions, SourceRegion::Code),
        comments: masked_view(src.as_bytes(), regions, SourceRegion::Comment),
    }
}
