//! Byte layout of C-family aggregates: `Type name;` fields with C scalar sizes.
use crate::metrics::FieldLayout;

fn estimate_type_layout(ty: &str) -> (usize, usize) {
    let trimmed = ty.trim();
    if trimmed.ends_with('*') || trimmed.contains("::size_t") {
        return (8, 8);
    }
    match trimmed {
        "uint64_t" | "int64_t" | "double" => (8, 8),
        "uint32_t" | "int32_t" | "int" | "float" | "unsigned" => (4, 4),
        "uint16_t" | "int16_t" | "short" => (2, 2),
        "bool" | "char" | "uint8_t" | "int8_t" => (1, 1),
        _ => (8, 8),
    }
}

/// Fields of a C-family aggregate body: one `Type name;` declaration per line.
pub fn parse_struct_fields(body: &str) -> Vec<FieldLayout> {
    let mut fields = Vec::new();
    let mut curr_offset = 0usize;
    for line in body.lines() {
        let Some(decl) = line.trim().strip_suffix(';') else {
            continue;
        };
        let Some((ty, name)) = decl.rsplit_once(' ') else {
            continue;
        };
        let (ty, name) = (ty.trim(), name.trim());
        if name.is_empty() || ty.is_empty() || ty.starts_with("return") || ty.ends_with(':') {
            continue;
        }
        let (size, align) = estimate_type_layout(ty);
        curr_offset = (curr_offset + align - 1) & !(align - 1);
        fields.push(FieldLayout {
            name: name.to_string(),
            ty: ty.to_string(),
            size,
            align,
            offset: curr_offset,
        });
        curr_offset += size;
    }
    fields
}

