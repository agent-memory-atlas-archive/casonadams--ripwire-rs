use crate::gate::baseline::QualitySnapshot;
use std::collections::HashMap;
use std::io::{self, Write};

#[derive(Debug, Clone, PartialEq)]
pub struct DmmProperty {
    pub key: &'static str,
    pub dmm: f64,
    pub good: u64,
    pub bad: u64,
    pub d_low: i64,
    pub d_high: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DmmReport {
    pub base_sha: String,
    pub target: String,
    pub at_stamp: String,
    pub dmm: f64,
    pub good: u64,
    pub bad: u64,
    pub base_units: usize,
    pub base_volume: u64,
    pub target_units: usize,
    pub target_volume: u64,
    pub properties: Vec<DmmProperty>,
}

impl DmmProperty {
    fn record_delta(&mut self, vol: u64, is_low_risk: bool, is_growth: bool) {
        let signed = if is_growth { vol as i64 } else { -(vol as i64) };
        if is_low_risk {
            self.d_low += signed;
        } else {
            self.d_high += signed;
        }
        if is_low_risk == is_growth {
            self.good += vol;
        } else {
            self.bad += vol;
        }
    }
    fn record_pair(&mut self, b_loc: Option<u32>, t_loc: Option<u32>, low_b: bool, low_t: bool) {
        match (b_loc, t_loc) {
            (None, Some(loc)) => self.record_delta(u64::from(loc), low_t, true),
            (Some(loc), None) => self.record_delta(u64::from(loc), low_b, false),
            (Some(b), Some(t)) if low_b != low_t => {
                self.record_delta(u64::from(b), low_b, false);
                self.record_delta(u64::from(t), low_t, true);
            }
            (Some(b), Some(t)) if t != b => {
                self.record_delta(u64::from(t.abs_diff(b)), low_t, t > b);
            }
            _ => {}
        }
    }
}

pub fn compute_dmm_property<F>(
    key: &'static str,
    base: &QualitySnapshot,
    target: &QualitySnapshot,
    is_low: F,
) -> DmmProperty
where
    F: Fn(&str, &QualitySnapshot) -> bool,
{
    let mut prop = DmmProperty {
        key,
        dmm: 1.0,
        good: 0,
        bad: 0,
        d_low: 0,
        d_high: 0,
    };
    let mut all_syms: HashMap<&str, (Option<u32>, Option<u32>)> = HashMap::new();
    for (sym, &loc) in &base.loc_by_sym {
        all_syms.entry(sym.as_str()).or_default().0 = Some(loc);
    }
    for (sym, &loc) in &target.loc_by_sym {
        all_syms.entry(sym.as_str()).or_default().1 = Some(loc);
    }
    for (sym, (b_loc, t_loc)) in all_syms {
        prop.record_pair(b_loc, t_loc, is_low(sym, base), is_low(sym, target));
    }
    if prop.good + prop.bad > 0 {
        prop.dmm = (prop.good as f64) / ((prop.good + prop.bad) as f64);
    }
    prop
}

pub fn compute_dmm_report(
    base: &QualitySnapshot,
    target: &QualitySnapshot,
    target_name: &str,
    at_stamp: &str,
) -> DmmReport {
    let p_size = compute_dmm_property("size", base, target, |s, snap| {
        snap.loc_by_sym.get(s).copied().unwrap_or(0) <= 15
    });
    let p_cx = compute_dmm_property("complexity", base, target, |s, snap| {
        snap.cx_by_sym.get(s).copied().unwrap_or(0) <= 5
    });
    let p_params = compute_dmm_property("interfacing", base, target, |s, snap| {
        snap.params_by_sym.get(s).copied().unwrap_or(0) <= 2
    });

    let good = p_size.good + p_cx.good + p_params.good;
    let bad = p_size.bad + p_cx.bad + p_params.bad;
    let dmm = if good + bad > 0 {
        (good as f64) / ((good + bad) as f64)
    } else {
        1.0
    };

    let base_units = base.loc_by_sym.len();
    let base_volume: u64 = base.loc_by_sym.values().map(|&v| v as u64).sum();
    let target_units = target.loc_by_sym.len();
    let target_volume: u64 = target.loc_by_sym.values().map(|&v| v as u64).sum();

    DmmReport {
        base_sha: base.head_sha.clone(),
        target: target_name.to_string(),
        at_stamp: at_stamp.to_string(),
        dmm,
        good,
        bad,
        base_units,
        base_volume,
        target_units,
        target_volume,
        properties: vec![p_size, p_cx, p_params],
    }
}

impl crate::serialize::XmlReport for DmmReport {
    crate::xml_report_meta!(
        "dmm",
        "ripwire-rs.dmm/v1",
        root: false,
        comment: "ripwire-rs dmm schema=ripwire-rs.dmm/v1: Delta Maintainability Model."
    );
    fn write_extra_attrs<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let at_attr = if !self.at_stamp.is_empty() {
            format!(" at=\"{}\"", self.at_stamp)
        } else {
            String::new()
        };
        let avail = usize::from(self.good + self.bad > 0);
        let dmm_str = if avail == 1 {
            format!("{:.3}", self.dmm)
        } else {
            "UNAVAILABLE".to_string()
        };
        let reason_attr = if avail == 0 {
            " reason=\"no unit's size, complexity or parameter count moved — this change is outside what the model measures\""
        } else {
            ""
        };
        write!(
            w,
            " base=\"{}\" target=\"{}\"{at_attr} available=\"{avail}\" combine=\"pooled\" size_metric=\"physical-loc\" low_loc=\"15\" low_cx=\"5\" low_params=\"2\" dmm=\"{dmm_str}\" good=\"{}\" bad=\"{}\" base_units=\"{}\" base_volume=\"{}\" target_units=\"{}\" target_volume=\"{}\"{reason_attr}",
            self.base_sha,
            self.target,
            self.good,
            self.bad,
            self.base_units,
            self.base_volume,
            self.target_units,
            self.target_volume
        )
    }
    fn write_body<W: Write>(&self, w: &mut W) -> io::Result<()> {
        for p in &self.properties {
            let p_dmm = if p.good + p.bad > 0 {
                format!("{:.3}", p.dmm)
            } else {
                "UNAVAILABLE".to_string()
            };
            writeln!(
                w,
                "  <p k=\"{}\" dmm=\"{p_dmm}\" good=\"{}\" bad=\"{}\" d_low=\"{}\" d_high=\"{}\"/>",
                p.key, p.good, p.bad, p.d_low, p.d_high
            )?;
        }
        Ok(())
    }
}

pub fn serialize_dmm_xml<W: Write>(rep: &DmmReport, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlReport;
    rep.render_envelope(out)
}
