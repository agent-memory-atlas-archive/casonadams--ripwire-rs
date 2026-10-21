use super::{ComplexityKinds, DialectGroup, LanguageSemantics, SyntaxKinds, VisibilityMode};
use crate::domain::Lang;

pub struct BrightScriptLanguage;
pub static BRIGHTSCRIPT_LANG: BrightScriptLanguage = BrightScriptLanguage;

const BRIGHTSCRIPT_STD_MEMBERS: &[&str] = &[
    "CreateObject",
    "GetInterface",
    "Type",
    "Val",
    "Str",
    "StrI",
    "Chr",
    "Asc",
    "Left",
    "Right",
    "Mid",
    "Len",
    "Instr",
    "LCase",
    "UCase",
    "Wait",
    "Sleep",
    "GetLastRunCompileError",
    "GetLastRunRuntimeError",
    "Rnd",
    "Box",
    "Unbox",
    "Run",
    "Eval",
    "GetGlobalAA",
    "FormatDrive",
    "ListDir",
    "ReadAsciiFile",
    "WriteAsciiFile",
    "CopyFile",
    "MoveFile",
    "MatchFiles",
    "DeleteFile",
    "DeleteDirectory",
    "CreateDirectory",
    "FindMemberFunction",
    "UpTime",
    "RebootSystem",
];

const BRIGHTSCRIPT_STD_QUALIFIERS: &[&str] = &[
    "m",
    "top",
    "global",
    "roSGNode",
    "roMessagePort",
    "roAssociativeArray",
    "roArray",
    "roDateTime",
    "roTimespan",
    "roRegex",
    "roString",
    "roInt",
    "roFloat",
    "roBoolean",
    "roByteArray",
    "roURLTransfer",
    "roDeviceInfo",
    "roAppInfo",
    "roAudioPlayer",
    "roVideoPlayer",
    "roLocalization",
    "roRegistry",
    "roRegistrySection",
    "roFileSystem",
    "roXMLElement",
];

const KINDS: ComplexityKinds = ComplexityKinds {
    paths: &[],
    flat: &["else_if_clause"],
    nesting: &[
        "single_line_if",
        "multi_line_if",
        "for_statement",
        "while_statement",
        "catch_clause",
    ],
    nest_only: &[],
    else_kinds: &["else"],
    else_bodies: &[],
    jumps: &[
        "exit_for_statement",
        "exit_while_statement",
        "continue_for_statement",
        "continue_while_statement",
        "return_statement",
        "throw_statement",
    ],
    logical_nodes: &[],
    binary_nodes: &["logical_expression"],
    logical_ops: &["and", "or"],
};

const SYNTAX: SyntaxKinds = SyntaxKinds {
    body_blocks: &["block"],
    body_wrappers: &[
        "variable_declarator",
        "parenthesized_expression",
        "assignment_statement",
    ],
    identifiers: &["identifier"],
    callable_wrappers: &["variable_declarator", "parenthesized_expression"],
    ..SyntaxKinds::NONE
};

impl LanguageSemantics for BrightScriptLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }
    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }

    fn tag(&self) -> &'static str {
        "brightscript"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["brs", "bs"]
    }

    super::impl_grammar_method!(
        ffi tree_sitter_brightscript,
        include_str!("../queries/brightscript/tags.scm")
    );

    fn display_color(&self) -> &'static str {
        "#662d91"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::BrightScript)
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::DefaultPublic
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_')
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        BRIGHTSCRIPT_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        BRIGHTSCRIPT_STD_QUALIFIERS
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        self.std_member_names()
            .iter()
            .any(|s| s.eq_ignore_ascii_case(name))
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        self.std_qualifier_names()
            .iter()
            .any(|s| s.eq_ignore_ascii_case(qualifier))
    }
}
