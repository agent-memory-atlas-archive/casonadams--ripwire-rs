use super::arise::{OccRole, VariableOcc};
use std::collections::{HashSet, VecDeque};

/// Direction of dataflow traversal.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SliceFlowDir {
    #[default]
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

impl std::fmt::Display for SliceFlowDir {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SliceFlowDir {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_opt(s).ok_or_else(|| format!("invalid slice flow direction: '{s}'"))
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
fn find_stmt_vars_matching(
    occs: &[VariableOcc],
    stmt_line: u32,
    current_var: &str,
    want_def: bool,
) -> Vec<String> {
    let mut out = Vec::new();
    for occ in occs {
        let matches_role = if want_def {
            occ.role.is_def()
        } else {
            occ.role.is_use()
        };
        if occ.stmt_line == stmt_line && matches_role && occ.var_name != current_var {
            out.push(occ.var_name.clone());
        }
    }
    out
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum TraversalDirection {
    Forward,
    Backward,
}

impl TraversalDirection {
    fn is_seed_role(self, role: OccRole) -> bool {
        match self {
            Self::Forward => role.is_def(),
            Self::Backward => role.is_use(),
        }
    }

    fn find_reaching<'a>(
        self,
        occs: &'a [VariableOcc],
        var: &str,
        line: u32,
    ) -> Vec<&'a VariableOcc> {
        match self {
            Self::Forward => find_reaching_uses(occs, var, line),
            Self::Backward => find_reaching_defs(occs, var, line),
        }
    }

    fn find_stmt_hop_vars(
        self,
        occs: &[VariableOcc],
        stmt_line: u32,
        current_var: &str,
    ) -> Vec<String> {
        match self {
            Self::Forward => find_stmt_vars_matching(occs, stmt_line, current_var, true),
            Self::Backward => find_stmt_vars_matching(occs, stmt_line, current_var, false),
        }
    }
}

struct FlowQueueItem {
    var: String,
    line: u32,
    depth: u32,
}

fn run_directional_flow(
    occs: &[VariableOcc],
    seed_var: &str,
    max_depth: u32,
    direction: TraversalDirection,
    steps: &mut Vec<SliceStep>,
) {
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();

    for occ in occs {
        if occ.var_name == seed_var && direction.is_seed_role(occ.role) {
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
        for target in direction.find_reaching(occs, &item.var, item.line) {
            expand_flow_target(
                occs,
                target,
                &item,
                direction,
                steps,
                &mut queue,
                &mut visited,
            );
        }
    }
}

fn expand_flow_target(
    occs: &[VariableOcc],
    target: &VariableOcc,
    item: &FlowQueueItem,
    direction: TraversalDirection,
    steps: &mut Vec<SliceStep>,
    queue: &mut VecDeque<FlowQueueItem>,
    visited: &mut HashSet<(String, u32, OccRole)>,
) {
    if visited.insert((target.var_name.clone(), target.line, target.role)) {
        steps.push(SliceStep {
            line: target.line,
            var_name: target.var_name.clone(),
            role: target.role,
            depth: item.depth + 1,
            from_line: item.line,
        });
        for next_var in direction.find_stmt_hop_vars(occs, target.stmt_line, &target.var_name) {
            queue.push_back(FlowQueueItem {
                var: next_var,
                line: target.line,
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
        run_directional_flow(
            occs,
            seed_var,
            max_depth,
            TraversalDirection::Forward,
            &mut steps,
        );
    }
    if dir == SliceFlowDir::Back || dir == SliceFlowDir::Both {
        run_directional_flow(
            occs,
            seed_var,
            max_depth,
            TraversalDirection::Backward,
            &mut steps,
        );
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
