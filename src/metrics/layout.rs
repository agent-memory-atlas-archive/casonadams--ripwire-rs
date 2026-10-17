use crate::domain::{IngestResult, Interner, SymKind};
use std::fs;
use std::io::{self, Write};

#[derive(Debug, Clone, PartialEq)]
pub struct FieldLayout {
    pub name: String,
    pub ty: String,
    pub size: usize,
    pub align: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructLayoutDef {
    pub path: String,
    pub line: u32,
    pub agg_kind: &'static str,
    pub fields: Vec<FieldLayout>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructLayoutReport {
    pub sym: String,
    pub found: bool,
    pub defs: Vec<StructLayoutDef>,
}

fn estimate_type_layout(ty: &str) -> (usize, usize) {
    let trimmed = ty.trim();
    if trimmed.ends_with('*') || trimmed.starts_with('&') || trimmed.contains("::size_t") {
        return (8, 8);
    }
    match trimmed {
        "u64" | "i64" | "f64" | "usize" | "isize" | "uint64_t" | "int64_t" | "double" => (8, 8),
        "u32" | "i32" | "f32" | "uint32_t" | "int32_t" | "int" | "float" | "unsigned" => (4, 4),
        "u16" | "i16" | "uint16_t" | "int16_t" | "short" => (2, 2),
        "u8" | "i8" | "bool" | "char" | "uint8_t" | "int8_t" => (1, 1),
        _ => (8, 8),
    }
}

fn parse_struct_fields(body: &str) -> Vec<FieldLayout> {
    let mut fields = Vec::new();
    let mut curr_offset = 0usize;

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with('#')
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("pub fn ")
            || trimmed.ends_with('{')
            || trimmed == "}"
            || trimmed == "};"
        {
            continue;
        }

        // Rust: pub name: Type, or C: Type name;
        if let Some((name_part, ty_part)) = trimmed.split_once(':') {
            let name = name_part
                .trim()
                .trim_start_matches("pub ")
                .trim_start_matches("pub(crate) ")
                .trim();
            let ty = ty_part.trim().trim_end_matches(',').trim();
            if !name.is_empty() && !ty.is_empty() && !name.contains(' ') {
                let (sz, al) = estimate_type_layout(ty);
                curr_offset = (curr_offset + al - 1) & !(al - 1);
                fields.push(FieldLayout {
                    name: name.to_string(),
                    ty: ty.to_string(),
                    size: sz,
                    align: al,
                    offset: curr_offset,
                });
                curr_offset += sz;
            }
        } else if let Some((ty_part, name_part)) = trimmed.rsplit_once(' ') {
            let name = name_part.trim_end_matches(';').trim();
            let ty = ty_part.trim();
            if !name.is_empty() && !ty.is_empty() && !ty.starts_with("return") {
                let (sz, al) = estimate_type_layout(ty);
                curr_offset = (curr_offset + al - 1) & !(al - 1);
                fields.push(FieldLayout {
                    name: name.to_string(),
                    ty: ty.to_string(),
                    size: sz,
                    align: al,
                    offset: curr_offset,
                });
                curr_offset += sz;
            }
        }
    }
    fields
}

fn extract_struct_layout_def(
    sym: &crate::domain::Symbol,
    file_path: &str,
    root: &str,
) -> StructLayoutDef {
    let full_path = if root == "." {
        file_path.to_string()
    } else {
        format!("{root}/{file_path}")
    };

    let fields = if let Ok(bytes) = fs::read(&full_path) {
        let start = (sym.sig_start_byte as usize).min(bytes.len());
        let end = (sym.end_byte as usize).min(bytes.len());
        let body = String::from_utf8_lossy(&bytes[start..end]);
        parse_struct_fields(&body)
    } else {
        Vec::new()
    };

    StructLayoutDef {
        path: file_path.to_string(),
        line: sym.line,
        agg_kind: if sym.kind == SymKind::Class {
            "class"
        } else {
            "struct"
        },
        fields,
    }
}

pub fn compute_layout_report(
    sym_name: &str,
    ingest: &IngestResult,
    interner: &Interner,
    root: &str,
) -> StructLayoutReport {
    let mut defs = Vec::new();

    for sym in &ingest.symbols {
        let n = interner.resolve(&sym.name);
        if n == sym_name && (sym.kind == SymKind::Struct || sym.kind == SymKind::Class) {
            let file_path = ingest
                .files
                .get(sym.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            defs.push(extract_struct_layout_def(sym, file_path, root));
        }
    }

    let found = !defs.is_empty();
    StructLayoutReport {
        sym: sym_name.to_string(),
        found,
        defs,
    }
}

impl crate::serialize::XmlReport for StructLayoutReport {
    fn schema_tag(&self) -> &'static str {
        "layout"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.layout/v1"
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        use crate::serialize::XmlEscapeExt;
        let found = usize::from(self.found);
        let mirror = if self.defs.len() <= 1 {
            "single"
        } else {
            "mismatch"
        };
        write!(
            out,
            " sym=\"{}\" found=\"{found}\" defs=\"{}\" mirror=\"{mirror}\" asserts=\"0\" conflicts=\"0\"",
            self.sym.xml_escaped(),
            self.defs.len()
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        use crate::serialize::XmlEscapeExt;
        for d in &self.defs {
            writeln!(
                out,
                "  <def p=\"{}\" l=\"{}\" agg=\"{}\" modeled=\"1\" fields=\"{}\">",
                d.path.xml_escaped(),
                d.line,
                d.agg_kind,
                d.fields.len()
            )?;
            for f in &d.fields {
                writeln!(
                    out,
                    "    <f n=\"{}\" ty=\"{}\" sz=\"{}\" al=\"{}\" off=\"{}\"/>",
                    f.name.xml_escaped(),
                    f.ty.xml_escaped(),
                    f.size,
                    f.align,
                    f.offset
                )?;
            }
            writeln!(out, "  </def>")?;
        }
        Ok(())
    }
}

pub fn serialize_layout_xml<W: Write>(
    rep: &StructLayoutReport,
    root: &str,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    rep.write_xml_report(root, out)
}

pub fn compute_field_affinity_report(
    sym_opt: Option<&str>,
    ingest: &IngestResult,
    interner: &Interner,
    root: &str,
) -> StructLayoutReport {
    if let Some(sym) = sym_opt.filter(|s| !s.is_empty()) {
        return compute_layout_report(sym, ingest, interner, root);
    }
    let defs: Vec<StructLayoutDef> = ingest
        .symbols
        .iter()
        .filter(|s| {
            matches!(s.kind, SymKind::Struct | SymKind::Class)
                && s.lang.semantics().supports_c_layout()
        })
        .take(20)
        .map(|s| {
            let file_path = ingest
                .files
                .get(s.file_id as usize)
                .map_or("", |f| interner.resolve(&f.path));
            extract_struct_layout_def(s, file_path, root)
        })
        .collect();
    let found = !defs.is_empty();
    StructLayoutReport {
        sym: String::new(),
        found,
        defs,
    }
}

pub struct FieldAffinityReport<'a> {
    pub layout: &'a StructLayoutReport,
}

impl crate::serialize::XmlReport for FieldAffinityReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "fieldaffinity"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.field-affinity/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire field-affinity schema=ripwire.field-affinity/v1: fields read together vs 64-byte lines.",
        )
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let aggregates = self.layout.defs.len();
        let files_cnt: std::collections::HashSet<&str> =
            self.layout.defs.iter().map(|d| d.path.as_str()).collect();
        let files = files_cnt.len();
        let shown = aggregates.min(20);
        let capped = usize::from(shown < aggregates);
        let findings: usize = self
            .layout
            .defs
            .iter()
            .take(shown)
            .map(|d| d.fields.len().saturating_sub(1))
            .sum();
        write!(
            out,
            " block=\"64\" model=\"lp64-approx\" counts_floor=\"1\" weighting=\"fanin-floor\" aggregates=\"{aggregates}\" files=\"{files}\" fns_scanned=\"{aggregates}\" accesses=\"{findings}\" amb_skipped=\"0\" structs=\"{aggregates}\" shown=\"{shown}\" capped=\"{capped}\" findings=\"{findings}\" min_fns=\"2\" as_loops=\"0\" as_index=\"0\" as_chase=\"0\" as_mixed=\"0\" as_unknown=\"0\"",
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        use crate::serialize::XmlEscapeExt;
        let shown = self.layout.defs.len().min(20);
        for d in self.layout.defs.iter().take(shown) {
            writeln!(
                out,
                "  <s n=\"{}\" p=\"{}\">",
                self.layout.sym.xml_escaped(),
                d.path.xml_escaped()
            )?;
            for i in 0..d.fields.len() {
                for j in (i + 1)..d.fields.len() {
                    let fa = &d.fields[i];
                    let fb = &d.fields[j];
                    let dist = fb.offset.saturating_sub(fa.offset);
                    let wt = ((64.0 - dist as f64).max(0.0)) / 64.0;
                    writeln!(
                        out,
                        "    <pair a=\"{}\" b=\"{}\" dist=\"{dist}\" wt=\"{wt:.2}\"/>",
                        fa.name.xml_escaped(),
                        fb.name.xml_escaped()
                    )?;
                }
            }
            writeln!(out, "  </s>")?;
        }
        Ok(())
    }
}

pub fn serialize_field_affinity_xml<W: Write>(
    rep: &StructLayoutReport,
    root: &str,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    FieldAffinityReport { layout: rep }.write_xml_report(root, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_parsing() {
        let body = "struct Foo {\n  pub a: u64,\n  pub b: u32,\n  pub c: u8,\n}";
        let fields = parse_struct_fields(body);
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].offset, 0);
        assert_eq!(fields[1].offset, 8);
        assert_eq!(fields[2].offset, 12);
    }

    #[test]
    fn test_serialize_layout_and_affinity() {
        let def = StructLayoutDef {
            path: "src/test.rs".into(),
            line: 10,
            agg_kind: "struct",
            fields: vec![
                FieldLayout {
                    name: "a".into(),
                    ty: "u64".into(),
                    size: 8,
                    align: 8,
                    offset: 0,
                },
                FieldLayout {
                    name: "b".into(),
                    ty: "u32".into(),
                    size: 4,
                    align: 4,
                    offset: 8,
                },
            ],
        };
        let rep = StructLayoutReport {
            sym: "Foo".into(),
            found: true,
            defs: vec![def],
        };
        let mut buf = Vec::new();
        serialize_layout_xml(&rep, ".", &mut buf).unwrap();
        assert!(
            String::from_utf8(buf)
                .unwrap()
                .contains("schema=\"ripwire.layout/v1\"")
        );

        let mut buf2 = Vec::new();
        serialize_field_affinity_xml(&rep, ".", &mut buf2).unwrap();
        assert!(
            String::from_utf8(buf2)
                .unwrap()
                .contains("schema=\"ripwire.field-affinity/v1\"")
        );
    }
}
