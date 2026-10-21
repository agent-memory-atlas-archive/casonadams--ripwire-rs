use crate::domain::Lang;

/// A definition a binding framework exposes to another language under `exported`
/// (e.g. pybind11 `m.def("add", &add_impl)` exposes `add_impl` to Python as `add`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignExport {
    pub exported: String,
    pub target: String,
    pub target_scope: Option<String>,
    pub caller: Lang,
}
