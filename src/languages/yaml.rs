use super::{DefCaptureGateContext, DialectGroup, LanguageSemantics};
use crate::domain::Lang;
use crate::ingest::{FileParseResult, yaml_guard};
use tree_sitter::{Language, Query};

pub struct YamlLanguage;
pub static YAML_LANG: YamlLanguage = YamlLanguage;

fn yaml_mapping_depth(mut curr: Option<tree_sitter::Node<'_>>) -> u32 {
    let mut depth = 0u32;
    while let Some(node) = curr {
        let k = node.kind();
        depth += u32::from(k == "block_mapping" || k == "flow_mapping");
        curr = node.parent();
    }
    depth
}

impl LanguageSemantics for YamlLanguage {
    fn tag(&self) -> &'static str {
        "yaml"
    }

    fn display_color(&self) -> &'static str {
        "#cb171e"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Yaml)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn supports_slice(&self) -> bool {
        false
    }

    fn supports_embedded_sql(&self) -> bool {
        false
    }

    fn max_file_size(&self, default_max: u64) -> u64 {
        default_max.min(crate::crawler::DEFAULT_YAML_CEILING)
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        ctx.sub != Some("yamlkey") || (ctx.name != "<<" && yaml_mapping_depth(ctx.def_node) <= 2)
    }

    fn parse_file_override(
        &self,
        source_bytes: &[u8],
        _rel_path: &str,
        _language: &Language,
        _query: &Query,
    ) -> Option<FileParseResult> {
        if yaml_guard::yaml_nests_too_deep(source_bytes) {
            Some(FileParseResult::default())
        } else {
            None
        }
    }
}
