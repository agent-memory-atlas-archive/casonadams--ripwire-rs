use super::c::{C_FAMILY_KEYWORDS, CPP_STD_MEMBERS, CPP_STD_QUALIFIERS, resolve_c_family_include};
use super::{DialectGroup, LanguageSemantics};
use std::collections::HashMap;

pub struct ObjCLanguage;
pub static OBJC_LANG: ObjCLanguage = ObjCLanguage;

const OBJC_FOUNDATION_MEMBERS: &[&str] = &[
    "alloc",
    "init",
    "new",
    "copy",
    "mutableCopy",
    "description",
    "hash",
    "isEqual",
    "count",
    "objectForKey",
    "setObject",
    "addObject",
    "removeObject",
    "removeAllObjects",
    "containsObject",
    "firstObject",
    "lastObject",
    "stringWithFormat",
    "UTF8String",
    "length",
    "boolValue",
    "intValue",
    "integerValue",
    "doubleValue",
];

const OBJC_FOUNDATION_QUALIFIERS: &[&str] = &[
    "NSObject",
    "NSString",
    "NSMutableString",
    "NSArray",
    "NSMutableArray",
    "NSDictionary",
    "NSMutableDictionary",
    "NSSet",
    "NSMutableSet",
    "NSNumber",
    "NSData",
    "NSMutableData",
    "NSURL",
    "NSURLRequest",
    "NSURLSession",
    "NSFileManager",
    "NSBundle",
    "NSThread",
    "NSNotificationCenter",
    "NSUserDefaults",
    "NSDate",
    "NSError",
    "NSJSONSerialization",
];

impl LanguageSemantics for ObjCLanguage {
    fn tag(&self) -> &'static str {
        "m"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["m", "mm"]
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::CFamily
    }

    fn supports_c_layout(&self) -> bool {
        true
    }

    fn is_builtin_keyword(&self, name: &str) -> bool {
        C_FAMILY_KEYWORDS.contains(&name)
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        CPP_STD_MEMBERS.contains(&name) || OBJC_FOUNDATION_MEMBERS.contains(&name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        CPP_STD_QUALIFIERS.contains(&qualifier) || OBJC_FOUNDATION_QUALIFIERS.contains(&qualifier)
    }

    fn resolve_import_target(
        &self,
        importer_path: &str,
        raw_target: &str,
        file_index: &HashMap<&str, u32>,
    ) -> Option<u32> {
        resolve_c_family_include(importer_path, raw_target, file_index)
    }
}
