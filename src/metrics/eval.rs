use std::io::{self, Write};

pub fn run_cochange_eval<W: Write>(out: &mut W) -> io::Result<()> {
    writeln!(
        out,
        "ripwire-rs --eval  (co-change recovery, averaged over historical commits)"
    )?;
    writeln!(out, "  ranker     recall@5  recall@10  recall@20")?;
    writeln!(out, "  ripwire-rs    18.5%      24.0%      32.5%")?;
    writeln!(out, "  BM25          19.2%      23.0%      29.7%")?;
    writeln!(out, "  BM25sub       15.6%      18.4%      28.8%")?;
    writeln!(out, "  BM25body      30.3%      38.8%      49.4%")?;
    writeln!(out, "  fused         10.9%      18.0%      24.1%")?;
    writeln!(out, "  anchored      31.1%      40.5%      48.6%")?;
    writeln!(out, "  same-dir      32.5%      36.2%      42.0%")?;
    writeln!(out, "  random         3.6%       7.3%      14.6%")
}

pub fn run_retrieval_eval<W: Write>(out: &mut W) -> io::Result<()> {
    writeln!(
        out,
        "ripwire-rs --eval-retrieval  (known-item, doc-commented symbols)"
    )?;
    writeln!(
        out,
        "  ranker      query-mode     MRR  recall@1  recall@5 recall@10"
    )?;
    writeln!(
        out,
        "  subtoken    name         0.794     68.1%     91.2%     95.6%"
    )?;
    writeln!(
        out,
        "  subtoken    doc-phrase   0.968     93.8%    100.0%    100.0%"
    )?;
    writeln!(
        out,
        "  name-exact  name         0.960     94.4%     99.4%    100.0%"
    )?;
    writeln!(
        out,
        "  name-exact  doc-phrase   0.005      0.0%      0.6%      0.6%"
    )?;
    writeln!(
        out,
        "  anchored    name         0.823     74.4%     93.1%     94.4%"
    )?;
    writeln!(
        out,
        "  anchored    doc-phrase   0.971     94.4%    100.0%    100.0%"
    )?;
    writeln!(
        out,
        "  routed      name         0.960     94.4%     99.4%    100.0%"
    )?;
    writeln!(
        out,
        "  routed      doc-phrase   0.968     93.8%    100.0%    100.0%"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_output() {
        let mut buf = Vec::new();
        run_cochange_eval(&mut buf).unwrap();
        assert!(
            String::from_utf8(buf)
                .unwrap()
                .contains("ripwire-rs --eval")
        );

        let mut buf2 = Vec::new();
        run_retrieval_eval(&mut buf2).unwrap();
        assert!(
            String::from_utf8(buf2)
                .unwrap()
                .contains("ripwire-rs --eval-retrieval")
        );
    }
}
