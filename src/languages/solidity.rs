use super::{DialectGroup, LanguageSemantics};
use crate::domain::Lang;

pub struct SolidityLanguage;
pub static SOLIDITY_LANG: SolidityLanguage = SolidityLanguage;

const SOLIDITY_STD_MEMBERS: &[&str] = &[
    "push",
    "pop",
    "transfer",
    "send",
    "call",
    "delegatecall",
    "staticcall",
    "encode",
    "encodePacked",
    "encodeWithSelector",
    "encodeWithSignature",
    "encodeCall",
    "decode",
    "concat",
];

const SOLIDITY_STD_QUALIFIERS: &[&str] =
    &["abi", "msg", "block", "tx", "address", "string", "bytes"];

impl LanguageSemantics for SolidityLanguage {
    fn tag(&self) -> &'static str {
        "sol"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Solidity)
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        SOLIDITY_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        SOLIDITY_STD_QUALIFIERS
    }
}
