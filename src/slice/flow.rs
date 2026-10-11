use super::arise::{OccRole, VariableOcc};
use std::collections::{HashSet, VecDeque};

/// Direction of dataflow traversal.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceFlowDir {
    Fwd = 0,
    Back,
    Both,
}

impl SliceFlowDir {
    #[must_use]
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "fwd" | "forward" => Some(Self::Fwd),
            "back" | "backward" => Some(Self::Back),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        const DIRS: [&str; 3] = ["fwd", "back", "both"];
        DIRS[self as usize]
    }
}

/// A single step in the dataflow slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceStep {
    pub line: u32,
    pub var_name: String,
    pub role: OccRole,
    pub depth: u32,
    pub from_line: u32,
}

fn find_reaching_uses<'a>(
    occs: &'a [VariableOcc],
    var: &str,
    def_line: u32,
) -> Vec<&'a VariableOcc> {
    let mut uses = Vec::new();
    for occ in occs {
        if occ.var_name != var || occ.line <= def_line {
            continue;
        }
        if occ.role.is_def() {
            break;
        }
        if occ.role.is_use() {
            uses.push(occ);
        }
    }
    uses
}

fn find_reaching_defs<'a>(
    occs: &'a [VariableOcc],
    var: &str,
    use_line: u32,
) -> Vec<&'a VariableOcc> {
    let mut last_def = None;
    for occ in occs {
        if occ.var_name != var {
            continue;
        }
        if occ.line >= use_line {
            break;
        }
        if occ.role.is_def() {
            last_def = Some(occ);
        }
    }
    last_def.into_iter().collect()
}

fn find_stmt_defined_vars(occs: &[VariableOcc], stmt_line: u32, current_var: &str) -> Vec<String> {
    let mut out = Vec::new();
    for occ in occs {
        if occ.stmt_line == stmt_line && occ.role.is_def() && occ.var_name != current_var {
            out.push(occ.var_name.clone());
        }
    }
    out
}

fn find_stmt_used_vars(occs: &[VariableOcc], stmt_line: u32, current_var: &str) -> Vec<String> {
    let mut out = Vec::new();
    for occ in occs {
        if occ.stmt_line == stmt_line && occ.role.is_use() && occ.var_name != current_var {
            out.push(occ.var_name.clone());
        }
    }
    out
}

struct FlowQueueItem {
    var: String,
    line: u32,
    depth: u32,
}

fn run_forward_flow(
    occs: &[VariableOcc],
    seed_var: &str,
    max_depth: u32,
    steps: &mut Vec<SliceStep>,
) {
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();

    for occ in occs {
        if occ.var_name == seed_var && occ.role.is_def() {
            queue.push_back(FlowQueueItem {
                var: seed_var.to_string(),
                line: occ.line,
                depth: 0,
            });
            steps.push(SliceStep {
                line: occ.line,
                var_name: seed_var.to_string(),
                role: occ.role,
                depth: 0,
                from_line: 0,
            });
            visited.insert((seed_var.to_string(), occ.line, occ.role));
        }
    }

    while let Some(item) = queue.pop_front() {
        if item.depth >= max_depth {
            continue;
        }
        for u in find_reaching_uses(occs, &item.var, item.line) {
            expand_forward_use(occs, u, &item, steps, &mut queue, &mut visited);
        }
    }
}

fn expand_forward_use(
    occs: &[VariableOcc],
    u: &VariableOcc,
    item: &FlowQueueItem,
    steps: &mut Vec<SliceStep>,
    queue: &mut VecDeque<FlowQueueItem>,
    visited: &mut HashSet<(String, u32, OccRole)>,
) {
    if visited.insert((u.var_name.clone(), u.line, u.role)) {
        steps.push(SliceStep {
            line: u.line,
            var_name: u.var_name.clone(),
            role: u.role,
            depth: item.depth + 1,
            from_line: item.line,
        });
        for def_var in find_stmt_defined_vars(occs, u.stmt_line, &u.var_name) {
            queue.push_back(FlowQueueItem {
                var: def_var,
                line: u.line,
                depth: item.depth + 1,
            });
        }
    }
}

fn run_backward_flow(
    occs: &[VariableOcc],
    seed_var: &str,
    max_depth: u32,
    steps: &mut Vec<SliceStep>,
) {
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();

    for occ in occs {
        if occ.var_name == seed_var && occ.role.is_use() {
            queue.push_back(FlowQueueItem {
                var: seed_var.to_string(),
                line: occ.line,
                depth: 0,
            });
            steps.push(SliceStep {
                line: occ.line,
                var_name: seed_var.to_string(),
                role: occ.role,
                depth: 0,
                from_line: 0,
            });
            visited.insert((seed_var.to_string(), occ.line, occ.role));
        }
    }

    while let Some(item) = queue.pop_front() {
        if item.depth >= max_depth {
            continue;
        }
        for d in find_reaching_defs(occs, &item.var, item.line) {
            expand_backward_def(occs, d, &item, steps, &mut queue, &mut visited);
        }
    }
}

fn expand_backward_def(
    occs: &[VariableOcc],
    d: &VariableOcc,
    item: &FlowQueueItem,
    steps: &mut Vec<SliceStep>,
    queue: &mut VecDeque<FlowQueueItem>,
    visited: &mut HashSet<(String, u32, OccRole)>,
) {
    if visited.insert((d.var_name.clone(), d.line, d.role)) {
        steps.push(SliceStep {
            line: d.line,
            var_name: d.var_name.clone(),
            role: d.role,
            depth: item.depth + 1,
            from_line: item.line,
        });
        for used_var in find_stmt_used_vars(occs, d.stmt_line, &d.var_name) {
            queue.push_back(FlowQueueItem {
                var: used_var,
                line: d.line,
                depth: item.depth + 1,
            });
        }
    }
}
/// Compute dataflow slice steps from the seed variable.
#[must_use]
pub fn compute_dataflow_slice(
    occs: &[VariableOcc],
    seed_var: &str,
    dir: SliceFlowDir,
    max_depth: u32,
) -> Vec<SliceStep> {
    let mut steps = Vec::new();

    if dir == SliceFlowDir::Fwd || dir == SliceFlowDir::Both {
        run_forward_flow(occs, seed_var, max_depth, &mut steps);
    }
    if dir == SliceFlowDir::Back || dir == SliceFlowDir::Both {
        run_backward_flow(occs, seed_var, max_depth, &mut steps);
    }

    steps.sort_by(|a, b| {
        a.depth
            .cmp(&b.depth)
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.var_name.cmp(&b.var_name))
    });
    steps.dedup_by(|a, b| a.line == b.line && a.var_name == b.var_name && a.role == b.role);
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_forward_and_backward() {
        let occs = vec![
            VariableOcc {
                line: 1,
                stmt_line: 1,
                start_byte: 0,
                end_byte: 1,
                role: OccRole::Decl,
                var_name: "x".into(),
            },
            VariableOcc {
                line: 3,
                stmt_line: 3,
                start_byte: 10,
                end_byte: 11,
                role: OccRole::Read,
                var_name: "x".into(),
            },
            VariableOcc {
                line: 3,
                stmt_line: 3,
                start_byte: 12,
                end_byte: 13,
                role: OccRole::Assign,
                var_name: "y".into(),
            },
            VariableOcc {
                line: 5,
                stmt_line: 5,
                start_byte: 20,
                end_byte: 21,
                role: OccRole::Read,
                var_name: "y".into(),
            },
        ];

        let fwd_steps = compute_dataflow_slice(&occs, "x", SliceFlowDir::Fwd, 8);
        assert_eq!(fwd_steps[0].line, 1);
        assert_eq!(fwd_steps[0].var_name, "x");
        assert_eq!(fwd_steps[1].line, 3);

        let back_steps = compute_dataflow_slice(&occs, "y", SliceFlowDir::Back, 8);
        assert!(back_steps.iter().any(|s| s.line == 3 && s.var_name == "y"));
    }
}
