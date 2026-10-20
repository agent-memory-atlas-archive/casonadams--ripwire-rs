use std::ffi::OsStr;
use std::process::Command;

/// `git -C <repo>` with `core.fsmonitor` forced off, so a repository-configured fsmonitor hook never runs.
#[must_use]
pub fn git_command(repo: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(["-c", "core.fsmonitor=false", "-C"]).arg(repo);
    cmd
}
