//! 布局引擎入口：统一导出核心类型与函数。

pub mod analyze;
pub mod config;
pub mod convert;
pub mod coord;
pub mod ir;
pub mod measure;
pub mod recognize;
pub mod solver;
pub mod sugiyama;
pub mod types;

use crate::ast::Diagram;
use crate::builder::layout::convert::Measure;
use crate::builder::layout::solver::solver_for;
use crate::builder::types::OutputConfig;

pub use config::LayoutConfig;
pub use convert::ToLayoutGraph;
pub use ir::{LayoutGraph, PlacedGraph};
pub use solver::{DirectedSolver, GroupedDirected, GridSolver, LayoutSolver, LinearSolver, SimpleSolver, SolverKind};

/// 主布局入口：从 AST 求解为 `PlacedGraph`（纯几何）。
///
/// 统一管线：AST → `LayoutGraph`（转换）→ 选 solver → `PlacedGraph`（求解）。
pub fn layout_diagram(
    diagram: &Diagram,
    layout_config: &LayoutConfig,
    output_config: &OutputConfig,
) -> PlacedGraph {
    let measure = Measure::new(output_config);
    let graph = diagram.to_layout_graph(&measure);
    match solver_for(diagram) {
        SolverKind::Directed => DirectedSolver::solve(&graph, layout_config),
        SolverKind::Grid => GridSolver::solve(&graph, layout_config),
        SolverKind::Linear => LinearSolver::solve(&graph, layout_config),
        SolverKind::Simple => SimpleSolver::solve(&graph, layout_config),
    }
}
