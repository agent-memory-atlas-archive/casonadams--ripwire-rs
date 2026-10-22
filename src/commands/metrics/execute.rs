use std::io::{self, Write};

pub fn execute_with_git_state_fmt<F>(
    root: &str,
    cutoff: Option<i64>,
    pretty: bool,
    json_out: bool,
    runner: F,
) -> io::Result<()>
where
    F: FnOnce(
        &[crate::git::history::CommitInfo],
        &crate::Workspace,
        &mut crate::serialize::XmlWriter<'_, io::StdoutLock<'_>>,
    ) -> io::Result<()>,
{
    let commits_vec = crate::git::history::walk_repository_history_scoped(root, cutoff).ok();
    let commits = commits_vec.as_deref().unwrap_or(&[]);
    let state = crate::Workspace::load(root)
        .ok_or_else(|| io::Error::other("failed to load repository"))?;
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = crate::serialize::XmlWriter::with_format(&mut stdout_lock, pretty, json_out);
    runner(commits, &state, &mut stdout)?;
    stdout.finish()
}

#[derive(Default, Clone, Copy)]
pub struct GitMetricsFlags<'a> {
    pub cochange: Option<Option<&'a str>>,
    pub cochange_groups: bool,
    pub cochange_recur: Option<u32>,
    pub hotspots: bool,
    pub since: Option<&'a str>,
    pub pretty: bool,
    pub json_out: bool,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub fn dispatch_git_metrics_with_flags(
    root: &str,
    flags: GitMetricsFlags<'_>,
) -> Option<io::Result<()>> {
    if flags.cochange.is_some() || flags.cochange_groups {
        let probed = flags.cochange.flatten();
        let cutoff = crate::git::history::resolve_cutoff_timestamp(flags.since, 18);
        return Some(execute_with_git_state_fmt(
            root,
            cutoff,
            flags.pretty,
            flags.json_out,
            |commits, state, out| execute_cochange_with_options(probed, flags, commits, state, out),
        ));
    }
    if flags.hotspots {
        let cutoff = crate::git::history::resolve_cutoff_timestamp(flags.since, 12);
        return Some(execute_with_git_state_fmt(
            root,
            cutoff,
            flags.pretty,
            flags.json_out,
            |commits, state, out| {
                let page = crate::serialize::PageRequest {
                    limit: flags.limit,
                    offset: flags.offset,
                };
                execute_hotspots_with(flags.since, commits, state, page, out)
            },
        ));
    }
    None
}
pub fn execute_hotspots_with<W: Write>(
    since: Option<&str>,
    commits: &[crate::git::history::CommitInfo],
    state: &crate::Workspace,
    page: crate::serialize::PageRequest,
    out: &mut W,
) -> io::Result<()> {
    crate::query::safety::execute_hotspots_with(since, commits, &state.safety_context(), page, out)
}

pub fn execute_cochange_with_options<W: Write>(
    file: Option<&str>,
    flags: GitMetricsFlags<'_>,
    commits: &[crate::git::history::CommitInfo],
    state: &crate::Workspace,
    out: &mut W,
) -> io::Result<()> {
    let options = crate::query::safety::CochangeOptions {
        since: flags.since,
        cochange_recur: flags.cochange_recur,
        cochange_groups: flags.cochange_groups,
        limit: flags.limit,
        offset: flags.offset,
    };
    crate::query::safety::execute_cochange_with_options(
        file,
        options,
        commits,
        &state.safety_context(),
        out,
    )
}
