use crate::domain::{IngestResult, Interner};
use std::collections::HashMap;
use std::io::{self, Write};

/// Metrics and worst function details for a single hotspot file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotspotFile {
    pub file_path: String,
    pub churn: u32,
    pub ccx_sum: u32,
    pub score: u64,
    pub top_fn_name: String,
    pub top_fn_ccx: u32,
    pub top_fn_line: u32,
}

/// Aggregated hotspots report for a repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotspotsReport {
    pub files: Vec<HotspotFile>,
    pub total_files: usize,
    pub ranked_count: usize,
    pub unranked_no_churn: usize,
    pub unranked_no_complexity: usize,
}

impl crate::serialize::XmlReport for HotspotsReport {
    fn schema_tag(&self) -> &'static str {
        "hotspots"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.hotspots/v1"
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire hotspots schema=ripwire.hotspots/v1: maintenance pain = churn x ccx over window=.",
        )
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let pw =
            crate::serialize::PageWindow::compute(self.ranked_count, 40, None, None).xml_attrs();
        write!(
            out,
            " window=\"12mo@HEAD\" files=\"{}\" ranked=\"{}\"{pw} unranked_no_churn=\"{}\" unranked_no_complexity=\"{}\" root=\".\" at=\"HEAD\"",
            self.total_files,
            self.ranked_count,
            self.unranked_no_churn,
            self.unranked_no_complexity
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        for f in self.files.iter().take(40) {
            writeln!(
                out,
                "  <f p=\"{}\" churn=\"{}\" ccx=\"{}\" score=\"{}\" top=\"{}\" top_ccx=\"{}\" top_l=\"{}\"/>",
                f.file_path,
                f.churn,
                f.ccx_sum,
                f.score,
                f.top_fn_name,
                f.top_fn_ccx,
                f.top_fn_line
            )?;
        }
        Ok(())
    }
}

struct FileSymbolStats {
    ccx_sum: u32,
    worst_name: String,
    worst_ccx: u32,
    worst_line: u32,
}

fn aggregate_file_symbols(ingest: &IngestResult, interner: &Interner) -> Vec<FileSymbolStats> {
    let mut stats = Vec::with_capacity(ingest.files.len());
    for f in &ingest.files {
        let mut ccx_sum = 0u32;
        let mut worst_name = String::new();
        let mut worst_ccx = 0u32;
        let mut worst_line = 0u32;

        for s in &ingest.symbols {
            if s.file_id == f.id {
                ccx_sum = ccx_sum.saturating_add(s.ccx);
                if s.ccx > worst_ccx || worst_name.is_empty() {
                    worst_ccx = s.ccx;
                    worst_name = interner.resolve(&s.name).to_string();
                    worst_line = s.line;
                }
            }
        }
        stats.push(FileSymbolStats {
            ccx_sum,
            worst_name,
            worst_ccx,
            worst_line,
        });
    }
    stats
}

/// Compute maintenance hotspots (churn * ccx) across repository files.
#[must_use]
pub fn compute_hotspots(
    ingest: &IngestResult,
    interner: &Interner,
    file_churn: &HashMap<String, u32>,
) -> HotspotsReport {
    let total_files = ingest.files.len();
    let file_stats = aggregate_file_symbols(ingest, interner);

    let mut ranked = Vec::new();
    let mut unranked_no_churn = 0usize;
    let mut unranked_no_complexity = 0usize;

    for (f, stats) in ingest.files.iter().zip(file_stats) {
        let path = interner.resolve(&f.path);
        let churn = file_churn.get(path).copied().unwrap_or(0);

        if churn == 0 {
            unranked_no_churn += 1;
        } else if stats.ccx_sum == 0 {
            unranked_no_complexity += 1;
        } else {
            let score = (churn as u64) * (stats.ccx_sum as u64);
            ranked.push(HotspotFile {
                file_path: path.to_string(),
                churn,
                ccx_sum: stats.ccx_sum,
                score,
                top_fn_name: stats.worst_name,
                top_fn_ccx: stats.worst_ccx,
                top_fn_line: stats.worst_line,
            });
        }
    }

    ranked.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.file_path.cmp(&b.file_path))
    });

    let ranked_count = ranked.len();
    HotspotsReport {
        files: ranked,
        total_files,
        ranked_count,
        unranked_no_churn,
        unranked_no_complexity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FileInfo, Lang, SymKind, Symbol};
    use crate::serialize::ToXmlString;

    #[test]
    fn test_compute_hotspots_ranking() {
        let interner = Interner::new();
        let mut ingest = IngestResult::new();

        let p1 = interner.intern("src/auth.rs");
        let p2 = interner.intern("src/util.rs");
        let p3 = interner.intern("src/idle.rs");

        ingest.files.push(FileInfo {
            id: 0,
            path: p1,
            lang: Lang::Rust,
            line_count: 100,
            byte_size: 2000,
        });
        ingest.files.push(FileInfo {
            id: 1,
            path: p2,
            lang: Lang::Rust,
            line_count: 50,
            byte_size: 1000,
        });
        ingest.files.push(FileInfo {
            id: 2,
            path: p3,
            lang: Lang::Rust,
            line_count: 20,
            byte_size: 500,
        });

        let mut s1 = Symbol::new(
            0,
            interner.intern("login"),
            interner.intern("login"),
            SymKind::Function,
            Lang::Rust,
            0,
            10,
        );
        s1.ccx = 20;

        let mut s2 = Symbol::new(
            1,
            interner.intern("helper"),
            interner.intern("helper"),
            SymKind::Function,
            Lang::Rust,
            1,
            5,
        );
        s2.ccx = 5;

        ingest.symbols.push(s1);
        ingest.symbols.push(s2);

        let mut file_churn = HashMap::new();
        file_churn.insert("src/auth.rs".to_string(), 10);
        file_churn.insert("src/util.rs".to_string(), 30);
        file_churn.insert("src/idle.rs".to_string(), 0);

        let report = compute_hotspots(&ingest, &interner, &file_churn);
        assert_eq!(report.total_files, 3);
        assert_eq!(report.ranked_count, 2);
        assert_eq!(report.unranked_no_churn, 1);

        // auth.rs: churn=10, ccx=20 -> score = 200
        // util.rs: churn=30, ccx=5  -> score = 150
        assert_eq!(report.files[0].file_path, "src/auth.rs");
        assert_eq!(report.files[0].score, 200);
        assert_eq!(report.files[0].top_fn_name, "login");
        assert_eq!(report.files[0].top_fn_ccx, 20);

        assert_eq!(report.files[1].file_path, "src/util.rs");
        assert_eq!(report.files[1].score, 150);

        let xml = report.to_xml_string();
        assert!(xml.contains(
            "<hotspots schema=\"ripwire.hotspots/v1\" window=\"12mo@HEAD\" files=\"3\" ranked=\"2\""
        ));
        assert!(xml.contains("<f p=\"src/auth.rs\" churn=\"10\" ccx=\"20\" score=\"200\" top=\"login\" top_ccx=\"20\" top_l=\"10\"/>"));
    }
}
