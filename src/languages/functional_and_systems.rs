use super::{DefCaptureGateContext, DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::{Lang, SymKind};

pub struct GoLanguage;
pub struct DartLanguage;
pub struct ElixirLanguage;
pub struct ErlangLanguage;
pub struct ZigLanguage;
pub struct SolidityLanguage;

pub static GO_LANG: GoLanguage = GoLanguage;
pub static DART_LANG: DartLanguage = DartLanguage;
pub static ELIXIR_LANG: ElixirLanguage = ElixirLanguage;
pub static ERLANG_LANG: ErlangLanguage = ErlangLanguage;
pub static ZIG_LANG: ZigLanguage = ZigLanguage;
pub static SOLIDITY_LANG: SolidityLanguage = SolidityLanguage;

impl LanguageSemantics for GoLanguage {
    fn tag(&self) -> &'static str {
        "go"
    }

    fn display_color(&self) -> &'static str {
        "#00add8"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Go)
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ByNameConvention
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        sym_name.chars().next().is_some_and(|c| c.is_uppercase())
    }

    fn check_node_visibility(&self, def_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> bool {
        let Some(name_node) = def_node.child_by_field_name("name") else {
            return false;
        };
        let Ok(text) = name_node.utf8_text(source_bytes) else {
            return false;
        };
        text.chars().next().is_some_and(|c| c.is_uppercase())
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_go_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_go_std_qualifier_name(qualifier)
    }
}

fn dart_following_body(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let mut sib = node.next_sibling();
    while let Some(s) = sib {
        if s.kind() == "function_body" {
            return Some(s);
        }
        if s.is_named() {
            break;
        }
        sib = s.next_sibling();
    }
    None
}

impl LanguageSemantics for DartLanguage {
    fn tag(&self) -> &'static str {
        "dart"
    }

    fn display_color(&self) -> &'static str {
        "#00b4ab"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Dart)
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ByNameConvention
    }

    fn resolve_def_body_and_end<'a>(
        &self,
        def_node: Option<tree_sitter::Node<'a>>,
        default_end: usize,
    ) -> (Option<tree_sitter::Node<'a>>, usize) {
        let Some(dn) = def_node else {
            return (None, default_end);
        };
        if let Some(body) = dart_following_body(dn) {
            return (Some(body), body.end_byte());
        }
        (Some(dn), default_end)
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        !(ctx.kind == SymKind::Function
            && ctx
                .def_node
                .and_then(|dn| dn.parent())
                .is_some_and(|p| p.kind() == "method_signature"))
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    }

    fn should_ignore_call(&self, _name_node: Option<tree_sitter::Node<'_>>, name: &str) -> bool {
        matches!(name, "print" | "identical" | "jsonEncode" | "jsonDecode")
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_dart_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_dart_std_qualifier_name(qualifier)
    }

    fn resolve_import_target(
        &self,
        importer_path: &str,
        raw_target: &str,
        file_index: &std::collections::HashMap<&str, u32>,
    ) -> Option<u32> {
        resolve_dart_import_target(importer_path, raw_target, file_index)
    }
}

impl LanguageSemantics for ElixirLanguage {
    fn tag(&self) -> &'static str {
        "ex"
    }

    fn display_color(&self) -> &'static str {
        "#6e4a7e"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Beam
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ByNameConvention
    }

    fn resolve_def_params_and_name(
        &self,
        def_node: Option<tree_sitter::Node<'_>>,
        source_bytes: &[u8],
        kind: SymKind,
        orig_name: &str,
    ) -> (u16, String) {
        let params = def_node
            .map(|n| crate::ingest::elixir_param_count(n, source_bytes))
            .unwrap_or(0);
        let name = if kind == SymKind::Function {
            format!("{orig_name}/{params}")
        } else {
            orig_name.to_string()
        };
        (params, name)
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        is_elixir_def_allowed(ctx)
    }

    fn should_ignore_call(&self, _name_node: Option<tree_sitter::Node<'_>>, name: &str) -> bool {
        is_elixir_ignored_call(name)
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_beam_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_beam_std_qualifier_name(qualifier)
    }
}

impl LanguageSemantics for ErlangLanguage {
    fn tag(&self) -> &'static str {
        "erl"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Beam
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_beam_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_beam_std_qualifier_name(qualifier)
    }
}

impl LanguageSemantics for ZigLanguage {
    fn tag(&self) -> &'static str {
        "zig"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Zig)
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_zig_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_zig_std_qualifier_name(qualifier)
    }
}

impl LanguageSemantics for SolidityLanguage {
    fn tag(&self) -> &'static str {
        "sol"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Solidity)
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_solidity_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_solidity_std_qualifier_name(qualifier)
    }
}

fn is_go_std_member_name(name: &str) -> bool {
    matches!(
        name,
        "Error"
            | "String"
            | "Read"
            | "Write"
            | "Close"
            | "Flush"
            | "Seek"
            | "Lock"
            | "Unlock"
            | "RLock"
            | "RUnlock"
            | "Add"
            | "Done"
            | "Wait"
            | "Load"
            | "Store"
            | "Swap"
            | "CompareAndSwap"
            | "Context"
            | "Deadline"
            | "Err"
            | "Value"
            | "Len"
            | "Less"
            | "MarshalJSON"
            | "UnmarshalJSON"
            | "Scan"
    )
}

fn is_go_std_qualifier_name(name: &str) -> bool {
    matches!(
        name,
        "fmt"
            | "os"
            | "io"
            | "strings"
            | "bytes"
            | "strconv"
            | "time"
            | "context"
            | "sync"
            | "atomic"
            | "errors"
            | "filepath"
            | "path"
            | "http"
            | "url"
            | "json"
            | "xml"
            | "sort"
            | "slices"
            | "maps"
            | "math"
            | "rand"
            | "reflect"
            | "regexp"
            | "bufio"
            | "log"
            | "slog"
            | "testing"
            | "exec"
            | "syscall"
            | "runtime"
            | "binary"
            | "hex"
            | "base64"
            | "sha256"
            | "tls"
            | "net"
            | "sql"
    )
}

fn is_dart_std_member_name(name: &str) -> bool {
    matches!(
        name,
        "add"
            | "addAll"
            | "remove"
            | "clear"
            | "contains"
            | "containsKey"
            | "map"
            | "where"
            | "expand"
            | "forEach"
            | "fold"
            | "reduce"
            | "toList"
            | "toSet"
            | "join"
            | "split"
            | "trim"
            | "substring"
            | "replaceAll"
            | "startsWith"
            | "endsWith"
            | "toLowerCase"
            | "toUpperCase"
            | "toString"
            | "compareTo"
            | "then"
            | "catchError"
            | "whenComplete"
            | "isAfter"
            | "isBefore"
            | "isAtSameMomentAs"
            | "difference"
            | "toUtc"
            | "toLocal"
            | "toIso8601String"
            | "firstWhere"
            | "singleWhere"
            | "any"
            | "every"
            | "take"
            | "skip"
            | "cast"
            | "asMap"
            | "putIfAbsent"
            | "update"
            | "removeWhere"
            | "wait"
            | "value"
            | "error"
            | "delayed"
            | "decode"
            | "encode"
    )
}

fn is_dart_std_qualifier_name(name: &str) -> bool {
    matches!(
        name,
        "Future"
            | "Stream"
            | "List"
            | "Map"
            | "Set"
            | "Iterable"
            | "String"
            | "int"
            | "double"
            | "num"
            | "bool"
            | "DateTime"
            | "Duration"
            | "Uri"
            | "RegExp"
            | "JsonCodec"
            | "json"
            | "utf8"
            | "math"
            | "Platform"
            | "File"
            | "Directory"
    )
}

fn resolve_dart_package_import(
    importer_path: &str,
    pkg_spec: &str,
    file_index: &std::collections::HashMap<&str, u32>,
) -> Option<u32> {
    let (pkg, subpath) = pkg_spec.split_once('/')?;
    if let Some((top_dir, _)) = importer_path.split_once('/') {
        let ws_cand = format!("{top_dir}/{pkg}/lib/{subpath}");
        if let Some(id) = super::probe_with_suffixes(&ws_cand, file_index, &[".dart"]) {
            return Some(id);
        }
    }
    for cand in [
        format!("packages/{pkg}/lib/{subpath}"),
        format!("{pkg}/lib/{subpath}"),
        format!("lib/{subpath}"),
    ] {
        if let Some(id) = super::probe_with_suffixes(&cand, file_index, &[".dart"]) {
            return Some(id);
        }
    }
    None
}

fn resolve_dart_import_target(
    importer_path: &str,
    raw_target: &str,
    file_index: &std::collections::HashMap<&str, u32>,
) -> Option<u32> {
    let clean = super::strip_import_quotes(raw_target);
    if clean.is_empty() || clean.starts_with("dart:") {
        return None;
    }
    if let Some(pkg_spec) = clean.strip_prefix("package:") {
        return resolve_dart_package_import(importer_path, pkg_spec, file_index);
    }
    super::resolve_relative_or_exact(importer_path, clean, file_index, &[".dart"])
}

fn is_beam_std_member_name(name: &str) -> bool {
    matches!(
        name,
        "get"
            | "put"
            | "delete"
            | "fetch"
            | "fetch!"
            | "merge"
            | "keys"
            | "values"
            | "has_key?"
            | "new"
            | "to_list"
            | "from_struct"
            | "update"
            | "update!"
            | "pop"
            | "take"
            | "drop"
            | "split"
            | "map"
            | "each"
            | "filter"
            | "reject"
            | "reduce"
            | "find"
            | "any?"
            | "all?"
            | "member?"
            | "count"
            | "empty?"
            | "concat"
            | "flat_map"
            | "join"
            | "sort"
            | "reverse"
            | "uniq"
            | "at"
            | "first"
            | "last"
            | "trim"
            | "downcase"
            | "upcase"
            | "replace"
            | "starts_with?"
            | "ends_with?"
            | "contains?"
            | "inspect"
            | "puts"
            | "info"
            | "debug"
            | "warn"
            | "warning"
            | "error"
    )
}

fn is_beam_std_qualifier_name(name: &str) -> bool {
    matches!(
        name,
        "Enum"
            | "Map"
            | "Keyword"
            | "List"
            | "String"
            | "Integer"
            | "Float"
            | "Tuple"
            | "MapSet"
            | "Stream"
            | "File"
            | "Path"
            | "IO"
            | "Process"
            | "Task"
            | "Agent"
            | "GenServer"
            | "Supervisor"
            | "Application"
            | "Logger"
            | "Kernel"
            | "System"
            | "DateTime"
            | "NaiveDateTime"
            | "Date"
            | "Time"
            | "URI"
            | "Regex"
            | "Base"
            | "Jason"
            | "ets"
            | "erlang"
            | "lists"
            | "maps"
            | "proplists"
            | "binary"
            | "io"
            | "file"
            | "timer"
    )
}

fn is_elixir_fn_keyword(target: &str) -> bool {
    matches!(
        target,
        "def" | "defp" | "defmacro" | "defmacrop" | "defguard" | "defguardp" | "defdelegate"
    )
}

fn is_elixir_typed_attr(name: &str) -> bool {
    matches!(
        name,
        "type" | "typep" | "opaque" | "callback" | "macrocallback"
    )
}

fn is_elixir_metadata_attr(name: &str) -> bool {
    is_elixir_typed_attr(name)
        || matches!(
            name,
            "doc"
                | "moduledoc"
                | "typedoc"
                | "spec"
                | "impl"
                | "behaviour"
                | "behavior"
                | "derive"
                | "enforce_keys"
                | "dialyzer"
                | "on_load"
                | "before_compile"
                | "after_compile"
                | "after_verify"
                | "optional_callbacks"
                | "compile"
                | "external_resource"
                | "vsn"
                | "file"
        )
}

fn is_elixir_ignored_call(name: &str) -> bool {
    is_elixir_fn_keyword(name)
        || matches!(
            name,
            "defmodule"
                | "defprotocol"
                | "defimpl"
                | "defstruct"
                | "defexception"
                | "defoverridable"
                | "alias"
                | "import"
                | "require"
                | "use"
                | "quote"
                | "unquote"
                | "unquote_splicing"
                | "case"
                | "cond"
                | "for"
                | "if"
                | "unless"
                | "with"
                | "receive"
                | "try"
                | "test"
        )
}

fn is_elixir_call_def_allowed(dn: tree_sitter::Node<'_>, ctx: &DefCaptureGateContext<'_>) -> bool {
    let Some(target) = dn
        .child_by_field_name("target")
        .and_then(|t| t.utf8_text(ctx.source_bytes).ok())
    else {
        return false;
    };
    if ctx.kind == SymKind::Section {
        return matches!(target, "defmodule" | "defprotocol" | "defimpl");
    }
    if target == "test" {
        return ctx.name_node.is_some_and(|n| n.kind() == "string");
    }
    is_elixir_fn_keyword(target) && !matches!(ctx.name, "unquote" | "when")
}

fn is_elixir_attr_def_allowed(dn: tree_sitter::Node<'_>, ctx: &DefCaptureGateContext<'_>) -> bool {
    if ctx.sub == Some("attribute") {
        return !is_elixir_metadata_attr(ctx.name);
    }
    dn.child_by_field_name("operand")
        .and_then(|op| op.child_by_field_name("target"))
        .and_then(|t| t.utf8_text(ctx.source_bytes).ok())
        .is_some_and(is_elixir_typed_attr)
}

fn is_elixir_def_allowed(ctx: &DefCaptureGateContext<'_>) -> bool {
    let Some(dn) = ctx.def_node else {
        return false;
    };
    match dn.kind() {
        "call" => is_elixir_call_def_allowed(dn, ctx),
        "unary_operator" => is_elixir_attr_def_allowed(dn, ctx),
        _ => true,
    }
}

fn is_zig_std_member_name(name: &str) -> bool {
    matches!(
        name,
        "init"
            | "deinit"
            | "alloc"
            | "free"
            | "create"
            | "destroy"
            | "dupe"
            | "append"
            | "appendSlice"
            | "pop"
            | "clearRetainingCapacity"
            | "clearAndFree"
            | "toOwnedSlice"
            | "get"
            | "put"
            | "remove"
            | "contains"
            | "iterator"
            | "next"
            | "format"
            | "print"
            | "read"
            | "write"
            | "close"
    )
}

fn is_zig_std_qualifier_name(name: &str) -> bool {
    matches!(
        name,
        "std"
            | "mem"
            | "fs"
            | "io"
            | "os"
            | "posix"
            | "fmt"
            | "math"
            | "debug"
            | "testing"
            | "heap"
            | "time"
            | "Thread"
            | "ArrayList"
            | "StringHashMap"
            | "AutoHashMap"
    )
}

fn is_solidity_std_member_name(name: &str) -> bool {
    matches!(
        name,
        "push"
            | "pop"
            | "transfer"
            | "send"
            | "call"
            | "delegatecall"
            | "staticcall"
            | "encode"
            | "encodePacked"
            | "encodeWithSelector"
            | "encodeWithSignature"
            | "encodeCall"
            | "decode"
            | "concat"
    )
}

fn is_solidity_std_qualifier_name(name: &str) -> bool {
    matches!(
        name,
        "abi" | "msg" | "block" | "tx" | "address" | "string" | "bytes"
    )
}
