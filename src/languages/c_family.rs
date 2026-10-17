use super::{DialectGroup, LanguageSemantics, resolve_relative_or_exact, strip_import_quotes};
use std::collections::HashMap;

pub struct CLanguage;
pub struct CppLanguage;
pub struct CudaLanguage;
pub struct MetalLanguage;
pub struct ObjCLanguage;

pub static C_LANG: CLanguage = CLanguage;
pub static CPP_LANG: CppLanguage = CppLanguage;
pub static CUDA_LANG: CudaLanguage = CudaLanguage;
pub static METAL_LANG: MetalLanguage = MetalLanguage;
pub static OBJC_LANG: ObjCLanguage = ObjCLanguage;

fn resolve_c_family_include(
    importer_path: &str,
    raw_target: &str,
    file_index: &HashMap<&str, u32>,
) -> Option<u32> {
    let trimmed = raw_target.trim();
    if trimmed.starts_with('<') {
        return None;
    }
    let clean = strip_import_quotes(trimmed);
    resolve_relative_or_exact(importer_path, clean, file_index, &[".h", ".hpp", ".hh"])
}
fn is_c_family_keyword(name: &str) -> bool {
    matches!(
        name,
        "static_cast"
            | "dynamic_cast"
            | "const_cast"
            | "reinterpret_cast"
            | "sizeof"
            | "alignof"
            | "decltype"
            | "typeid"
            | "static_assert"
            | "noexcept"
    )
}

impl LanguageSemantics for CLanguage {
    fn tag(&self) -> &'static str {
        "c"
    }

    fn display_color(&self) -> &'static str {
        "#555555"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::CFamily
    }

    fn supports_c_layout(&self) -> bool {
        true
    }
    fn is_builtin_keyword(&self, name: &str) -> bool {
        is_c_family_keyword(name)
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

impl LanguageSemantics for CppLanguage {
    fn tag(&self) -> &'static str {
        "cpp"
    }

    fn display_color(&self) -> &'static str {
        "#f34b7d"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::CFamily
    }

    fn supports_c_layout(&self) -> bool {
        true
    }
    fn is_builtin_keyword(&self, name: &str) -> bool {
        is_c_family_keyword(name)
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

impl LanguageSemantics for CudaLanguage {
    fn tag(&self) -> &'static str {
        "cuda"
    }

    fn display_color(&self) -> &'static str {
        "#3a87ad"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::CFamily
    }

    fn supports_c_layout(&self) -> bool {
        true
    }
    fn is_builtin_keyword(&self, name: &str) -> bool {
        is_c_family_keyword(name)
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

impl LanguageSemantics for MetalLanguage {
    fn tag(&self) -> &'static str {
        "metal"
    }

    fn display_color(&self) -> &'static str {
        "#3a87ad"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::CFamily
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

impl LanguageSemantics for ObjCLanguage {
    fn tag(&self) -> &'static str {
        "m"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::CFamily
    }

    fn supports_c_layout(&self) -> bool {
        true
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
