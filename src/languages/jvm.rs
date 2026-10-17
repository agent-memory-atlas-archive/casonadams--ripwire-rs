use super::{DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::Lang;

pub struct JavaLanguage;
pub struct KotlinLanguage;
pub struct ScalaLanguage;
pub struct CSharpLanguage;
pub struct SwiftLanguage;
pub struct PhpLanguage;

pub static JAVA_LANG: JavaLanguage = JavaLanguage;
pub static KOTLIN_LANG: KotlinLanguage = KotlinLanguage;
pub static SCALA_LANG: ScalaLanguage = ScalaLanguage;
pub static CSHARP_LANG: CSharpLanguage = CSharpLanguage;
pub static SWIFT_LANG: SwiftLanguage = SwiftLanguage;
pub static PHP_LANG: PhpLanguage = PhpLanguage;

impl LanguageSemantics for JavaLanguage {
    fn tag(&self) -> &'static str {
        "java"
    }

    fn display_color(&self) -> &'static str {
        "#b07219"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Jvm
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }

    fn is_entrypoint_symbol(
        &self,
        sym_name: &str,
        _scope: Option<&str>,
        kind: crate::domain::SymKind,
        rel_path: &str,
    ) -> bool {
        kind.is_callable() && !super::is_non_entrypoint_path(rel_path) && sym_name == "main"
    }
    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_jvm_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_jvm_std_qualifier_name(qualifier)
    }
}

impl LanguageSemantics for KotlinLanguage {
    fn tag(&self) -> &'static str {
        "kt"
    }

    fn display_color(&self) -> &'static str {
        "#a97bff"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Jvm
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_jvm_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_jvm_std_qualifier_name(qualifier)
    }
}

impl LanguageSemantics for ScalaLanguage {
    fn tag(&self) -> &'static str {
        "scala"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Jvm
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_jvm_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_jvm_std_qualifier_name(qualifier)
    }
}

impl LanguageSemantics for CSharpLanguage {
    fn tag(&self) -> &'static str {
        "cs"
    }

    fn display_color(&self) -> &'static str {
        "#178600"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::CSharp)
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }

    fn is_entrypoint_symbol(
        &self,
        sym_name: &str,
        _scope: Option<&str>,
        kind: crate::domain::SymKind,
        rel_path: &str,
    ) -> bool {
        kind.is_callable()
            && !super::is_non_entrypoint_path(rel_path)
            && (sym_name == "main" || sym_name == "Main")
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_csharp_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_csharp_std_qualifier_name(qualifier)
    }
}

impl LanguageSemantics for SwiftLanguage {
    fn tag(&self) -> &'static str {
        "swift"
    }

    fn display_color(&self) -> &'static str {
        "#f05138"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Swift)
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_swift_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_swift_std_qualifier_name(qualifier)
    }
}

impl LanguageSemantics for PhpLanguage {
    fn tag(&self) -> &'static str {
        "php"
    }

    fn display_color(&self) -> &'static str {
        "#4f5d95"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Php)
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_php_std_member_name(name)
    }
}

fn is_jvm_std_member_name(name: &str) -> bool {
    matches!(
        name,
        "get"
            | "set"
            | "put"
            | "remove"
            | "add"
            | "addAll"
            | "clear"
            | "contains"
            | "containsKey"
            | "size"
            | "isEmpty"
            | "iterator"
            | "stream"
            | "map"
            | "filter"
            | "flatMap"
            | "forEach"
            | "collect"
            | "toList"
            | "toArray"
            | "orElse"
            | "orElseGet"
            | "orElseThrow"
            | "isPresent"
            | "equals"
            | "hashCode"
            | "toString"
            | "compareTo"
            | "length"
            | "charAt"
            | "substring"
            | "split"
            | "trim"
            | "replace"
            | "startsWith"
            | "endsWith"
            | "toLowerCase"
            | "toUpperCase"
            | "format"
            | "valueOf"
            | "parseInt"
            | "parseLong"
            | "append"
    )
}

fn is_jvm_std_qualifier_name(name: &str) -> bool {
    matches!(
        name,
        "System"
            | "Math"
            | "String"
            | "Integer"
            | "Long"
            | "Double"
            | "Boolean"
            | "Objects"
            | "Collections"
            | "Arrays"
            | "Optional"
            | "Stream"
            | "Collectors"
            | "List"
            | "Map"
            | "Set"
            | "Files"
            | "Paths"
            | "Path"
            | "UUID"
            | "Instant"
            | "Duration"
            | "LocalDate"
            | "LocalDateTime"
            | "CompletableFuture"
            | "Thread"
            | "Executors"
            | "LoggerFactory"
    )
}

fn is_csharp_std_member_name(name: &str) -> bool {
    matches!(
        name,
        "Add"
            | "AddRange"
            | "Remove"
            | "Clear"
            | "Contains"
            | "ContainsKey"
            | "TryGetValue"
            | "ToArray"
            | "ToList"
            | "ToDictionary"
            | "ToHashSet"
            | "Select"
            | "Where"
            | "FirstOrDefault"
            | "SingleOrDefault"
            | "Any"
            | "All"
            | "Count"
            | "ToString"
            | "Equals"
            | "GetHashCode"
            | "GetType"
            | "Substring"
            | "Split"
            | "Trim"
            | "Replace"
            | "StartsWith"
            | "EndsWith"
            | "ToLower"
            | "ToUpper"
            | "Format"
            | "Parse"
            | "TryParse"
            | "Append"
            | "Dispose"
            | "ConfigureAwait"
    )
}

fn is_csharp_std_qualifier_name(name: &str) -> bool {
    matches!(
        name,
        "Console"
            | "Math"
            | "String"
            | "Int32"
            | "Int64"
            | "Double"
            | "Boolean"
            | "Guid"
            | "DateTime"
            | "DateTimeOffset"
            | "TimeSpan"
            | "Task"
            | "Enumerable"
            | "Array"
            | "Convert"
            | "Environment"
            | "File"
            | "Directory"
            | "Path"
            | "JsonSerializer"
            | "Regex"
            | "Interlocked"
            | "GC"
    )
}

fn is_swift_std_member_name(name: &str) -> bool {
    matches!(
        name,
        "append"
            | "insert"
            | "remove"
            | "removeAll"
            | "contains"
            | "map"
            | "compactMap"
            | "flatMap"
            | "filter"
            | "reduce"
            | "forEach"
            | "first"
            | "last"
            | "sorted"
            | "reversed"
            | "split"
            | "joined"
            | "lowercased"
            | "uppercased"
            | "hasPrefix"
            | "hasSuffix"
    )
}

fn is_swift_std_qualifier_name(name: &str) -> bool {
    matches!(
        name,
        "String"
            | "Int"
            | "Double"
            | "Bool"
            | "Array"
            | "Dictionary"
            | "Set"
            | "Date"
            | "URL"
            | "Data"
            | "UUID"
            | "FileManager"
            | "UserDefaults"
            | "NotificationCenter"
            | "DispatchQueue"
            | "Task"
            | "MainActor"
            | "JSONEncoder"
            | "JSONDecoder"
    )
}

fn is_php_std_member_name(name: &str) -> bool {
    matches!(
        name,
        "count"
            | "sizeof"
            | "in_array"
            | "array_map"
            | "array_filter"
            | "array_merge"
            | "array_keys"
            | "array_values"
            | "array_push"
            | "array_pop"
            | "array_shift"
            | "array_slice"
            | "implode"
            | "explode"
            | "trim"
            | "strlen"
            | "strpos"
            | "substr"
            | "str_replace"
            | "strtolower"
            | "strtoupper"
            | "sprintf"
            | "json_encode"
            | "json_decode"
    )
}
