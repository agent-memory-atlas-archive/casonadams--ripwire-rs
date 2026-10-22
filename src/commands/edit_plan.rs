//! `--edit-plan` / `--edit-payload`: executes a versioned edit plan or JSON edit payload.
use super::common::safety::{context_of, report_exit};
use super::common::safety_queries::execute_edit_plan_dispatch;
use super::{Plugin, Run};
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct EditPlanArgs {
    /// Execute versioned edit plan
    #[arg(long = "edit-plan", value_name = "PLAN_JSON")]
    pub edit_plan: Option<String>,
    /// Execute JSON edit payload
    #[arg(long = "edit-payload", value_name = "PAYLOAD_JSON")]
    pub edit_payload: Option<String>,
    /// Validate edits without mutating files
    #[arg(long = "dry-run")]
    pub dry_run: bool,
    /// Apply atomic code mutations
    #[arg(long = "apply")]
    pub apply: bool,
    /// Force edits bypassing safety checks
    #[arg(long = "force")]
    pub force: bool,
    /// Target file for symbol edit disambiguation
    #[arg(long = "edit-target-file", value_name = "FILE")]
    pub edit_target_file: Option<String>,
    /// Skip post-edit contract and syntax verification
    #[arg(long = "no-post-check")]
    pub no_post_check: bool,
    /// Allow atomic symbol edits when connected over remote transport
    #[arg(long = "allow-remote-edits")]
    pub allow_remote_edits: bool,
}

fn run(run: &Run<'_>, args: &EditPlanArgs) -> io::Result<()> {
    let Some(plan_spec) = args.edit_plan.as_deref().or(args.edit_payload.as_deref()) else {
        return Ok(());
    };
    report_exit(run, |ctx, out| {
        let safety = context_of(ctx);
        execute_edit_plan_dispatch(plan_spec, &safety, args.dry_run, out).map(Some)
    })
}

pub static EDIT_PLAN: Plugin<EditPlanArgs> = Plugin {
    name: "edit-plan",
    options: &[],
    selected: |args| args.edit_plan.is_some() || args.edit_payload.is_some(),
    run,
    check: None,
    tools: &[],
};
