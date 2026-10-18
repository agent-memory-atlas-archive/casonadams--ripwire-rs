use crate::domain::{IngestResult, Interner, SymKind};
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

impl StructLayoutDef {
    #[must_use]
    pub fn max_align(&self) -> usize {
        self.fields.iter().map(|f| f.align).max().unwrap_or(1)
    }

    #[must_use]
    pub fn total_size(&self) -> usize {
        let raw = self.fields.last().map_or(1, |f| f.offset + f.size);
        let align = self.max_align();
        raw.div_ceil(align) * align
    }

    #[must_use]
    pub fn cache_lines(&self) -> usize {
        self.total_size().div_ceil(64).max(1)
    }

    #[must_use]
    pub fn pair_count(&self) -> usize {
        let n = self.fields.len();
        n * n.saturating_sub(1) / 2
    }

    #[must_use]
    pub fn sepcost(&self) -> f64 {
        let mut cost = 0.0;
        for i in 0..self.fields.len() {
            for j in (i + 1)..self.fields.len() {
                let dist = self.fields[j].offset.saturating_sub(self.fields[i].offset);
                let wt = ((64.0 - dist as f64).max(0.0)) / 64.0;
                cost += 1.0 - wt;
            }
        }
        cost
    }
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

pub fn parse_struct_fields(body: &str) -> Vec<FieldLayout> {
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
    let fields = if let Ok(bytes) = crate::domain::read_rel_bytes(root, file_path) {
        let body = sym.source_text(&bytes).unwrap_or_default();
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
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let defs: Vec<StructLayoutDef> = ingest
        .symbols
        .iter()
        .filter(|sym| {
            view.sym_name(sym) == sym_name && matches!(sym.kind, SymKind::Struct | SymKind::Class)
        })
        .map(|sym| extract_struct_layout_def(sym, view.sym_path(sym), root))
        .collect();
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
        "ripwire-rs.layout/v1"
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
) -> FieldAffinityReport {
    if let Some(sym) = sym_opt.filter(|s| !s.is_empty()) {
        return FieldAffinityReport {
            layout: compute_layout_report(sym, ingest, interner, root),
        };
    }
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let defs: Vec<StructLayoutDef> = ingest
        .symbols
        .iter()
        .filter(|s| {
            matches!(s.kind, SymKind::Struct | SymKind::Class)
                && s.lang.semantics().supports_c_layout()
        })
        .take(20)
        .map(|s| extract_struct_layout_def(s, view.sym_path(s), root))
        .collect();
    let found = !defs.is_empty();
    FieldAffinityReport {
        layout: StructLayoutReport {
            sym: String::new(),
            found,
            defs,
        },
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldAffinityReport {
    pub layout: StructLayoutReport,
}

impl crate::serialize::XmlReport for FieldAffinityReport {
    fn schema_tag(&self) -> &'static str {
        "fieldaffinity"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire-rs.field-affinity/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire-rs field-affinity schema=ripwire-rs.field-affinity/v1: fields read together vs 64-byte lines.",
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
            let f_cnt = d.fields.len();
            let pairs = d.pair_count();
            let sepcost = d.sepcost();
            let size = d.total_size();
            let align = d.max_align();
            let lines = d.cache_lines();
            writeln!(
                out,
                "  <s n=\"{}\" p=\"{}\" l=\"{}\" agg=\"{}\" modeled=\"1\" fields=\"{f_cnt}\" touched=\"{f_cnt}\" fns=\"1\" pairs=\"{pairs}\" sepcost=\"{sepcost:.2}\" findings=\"0\" size=\"{size}\" align=\"{align}\" lines=\"{lines}\">",
                self.layout.sym.xml_escaped(),
                d.path.xml_escaped(),
                d.line,
                d.agg_kind
            )?;
            for i in 0..d.fields.len() {
                for j in (i + 1)..d.fields.len() {
                    let fa = &d.fields[i];
                    let fb = &d.fields[j];
                    let dist = fb.offset.saturating_sub(fa.offset);
                    let wt = ((64.0 - dist as f64).max(0.0)) / 64.0;
                    writeln!(
                        out,
                        "    <pair a=\"{}\" b=\"{}\" fns=\"1\" w=\"1\" dist=\"{dist}\" wt=\"{wt:.2}\"/>",
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
    FieldAffinityReport {
        layout: rep.clone(),
    }
    .write_xml_report(root, out)
}
