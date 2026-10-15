use super::{DialectGroup, LanguageSemantics};
use crate::domain::Lang;
use crate::ingest::{FileParseResult, markdown, yaml_guard};
use tree_sitter::{Language, Query};

pub struct JsonLanguage;
pub struct TomlLanguage;
pub struct YamlLanguage;
pub struct MarkdownLanguage;
pub struct HclLanguage;
pub struct SqlLanguage;
pub struct ProtoLanguage;
pub struct GraphqlLanguage;
pub struct DockerfileLanguage;
pub struct UnknownLanguage;

pub static JSON_LANG: JsonLanguage = JsonLanguage;
pub static TOML_LANG: TomlLanguage = TomlLanguage;
pub static YAML_LANG: YamlLanguage = YamlLanguage;
pub static MARKDOWN_LANG: MarkdownLanguage = MarkdownLanguage;
pub static HCL_LANG: HclLanguage = HclLanguage;
pub static SQL_LANG: SqlLanguage = SqlLanguage;
pub static PROTO_LANG: ProtoLanguage = ProtoLanguage;
pub static GRAPHQL_LANG: GraphqlLanguage = GraphqlLanguage;
pub static DOCKERFILE_LANG: DockerfileLanguage = DockerfileLanguage;
pub static UNKNOWN_LANG: UnknownLanguage = UnknownLanguage;

impl LanguageSemantics for JsonLanguage {
    fn tag(&self) -> &'static str {
        "json"
    }

    fn display_color(&self) -> &'static str {
        "#292929"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Json)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn supports_slice(&self) -> bool {
        false
    }

    fn supports_embedded_sql(&self) -> bool {
        false
    }
}

impl LanguageSemantics for TomlLanguage {
    fn tag(&self) -> &'static str {
        "toml"
    }

    fn display_color(&self) -> &'static str {
        "#9c4221"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Toml)
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

impl LanguageSemantics for MarkdownLanguage {
    fn tag(&self) -> &'static str {
        "md"
    }

    fn display_color(&self) -> &'static str {
        "#083fa1"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Markdown)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn supports_slice(&self) -> bool {
        false
    }

    fn supports_embedded_sql(&self) -> bool {
        false
    }

    fn is_prose_document(&self) -> bool {
        true
    }

    fn parse_file_override(
        &self,
        source_bytes: &[u8],
        rel_path: &str,
        language: &Language,
        _query: &Query,
    ) -> Option<FileParseResult> {
        Some(markdown::extract_markdown(source_bytes, rel_path, language))
    }
}

impl LanguageSemantics for HclLanguage {
    fn tag(&self) -> &'static str {
        "hcl"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Hcl)
    }

    fn supports_slice(&self) -> bool {
        false
    }
}

impl LanguageSemantics for SqlLanguage {
    fn tag(&self) -> &'static str {
        "sql"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Sql)
    }

    fn supports_slice(&self) -> bool {
        false
    }

    fn supports_embedded_sql(&self) -> bool {
        false
    }

    fn is_table_read_write_target(&self) -> bool {
        true
    }
}

impl LanguageSemantics for ProtoLanguage {
    fn tag(&self) -> &'static str {
        "proto"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Proto)
    }

    fn supports_slice(&self) -> bool {
        false
    }
}

impl LanguageSemantics for GraphqlLanguage {
    fn tag(&self) -> &'static str {
        "gql"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Graphql)
    }

    fn supports_slice(&self) -> bool {
        false
    }
}

impl LanguageSemantics for DockerfileLanguage {
    fn tag(&self) -> &'static str {
        "docker"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Dockerfile)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn supports_slice(&self) -> bool {
        false
    }
}

impl LanguageSemantics for UnknownLanguage {
    fn tag(&self) -> &'static str {
        "?"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Unknown)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn supports_slice(&self) -> bool {
        false
    }
}
