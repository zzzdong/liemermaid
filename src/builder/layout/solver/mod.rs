//! 求解层：把 [`LayoutGraph`] 求解为 [`PlacedGraph`]。
//!
//! 求解器与 AST 完全解耦，只读纯拓扑的 `LayoutGraph`。四类求解器：
//! - [`DirectedSolver`]：flowchart / state（有向图分层）
//! - [`GroupedDirected`]：带子图 / 复合状态的有向图（递归 + 平移回贴）
//! - [`GridSolver`]：class / er（网格排布）
//! - [`LinearSolver`]：sequence / timeline（线性排布）
//! - [`SimpleSolver`]：pie / gitgraph（画布中心 / 分支列）

pub mod directed;
pub mod grouped;
pub mod grid;
pub mod linear;
pub mod simple;

use crate::ast::Diagram;

use super::config::LayoutConfig;
use super::ir::{LayoutGraph, PlacedGraph};

pub use directed::DirectedSolver;
pub use grouped::GroupedDirected;
pub use grid::GridSolver;
pub use linear::LinearSolver;
pub use simple::SimpleSolver;

/// 求解器统一入口。
pub trait LayoutSolver {
    fn solve(&self, graph: &LayoutGraph, config: &LayoutConfig) -> PlacedGraph;
}

/// 根据图表类型选择求解器种类。
pub enum SolverKind {
    Directed,
    Grid,
    Linear,
    Simple,
}

/// 按图表类型分派求解器种类。
pub fn solver_for(diagram: &Diagram) -> SolverKind {
    match diagram {
        Diagram::Flowchart(_) | Diagram::State(_) => SolverKind::Directed,
        Diagram::Class(_) | Diagram::Er(_) => SolverKind::Grid,
        Diagram::Sequence(_) | Diagram::Timeline(_) => SolverKind::Linear,
        Diagram::Pie(_) | Diagram::GitGraph(_) => SolverKind::Simple,
    }
}
