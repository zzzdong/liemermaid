//! 渲染层：把 `PlacedGraph`（纯几何）+ AST（形状/标签/箭头）渲染成 `SceneNode`。
//!
//! 渲染层只**读**坐标，不修改坐标。它拿 `PlacedGraph` 后回查 AST 取形状 / 文本 / 箭头，
//! 组合绘制。这是「求解」（改坐标）与「绘制」（读坐标）解耦的边界。
//!
//! # 分类渲染器（按 SolverKind 分派）
//!
//! 渲染入口 `render_placed` 按 `solver_for(diagram)` 得到的 `SolverKind` 分派到
//! 4 个家族渲染器，而不是按 `Diagram` 的具体类型逐一直派：
//!
//! - `DirectedRenderer`：`Flowchart` / `State`（几何完全由 `PlacedGraph` 表达）
//! - `GridRenderer`：`Class` / `Er`（领域专属几何，由各自 render 函数基于 AST 计算）
//! - `LinearRenderer`：`Sequence` / `Timeline`（生命线 / 时间轴分栏几何）
//! - `SimpleRenderer`：`Pie` / `GitGraph`（扇形 / DAG 几何）
//!
//! 属于同一家族的图表共用一个聚合模块，新增图表只需把它归入对应家族并在
//! 聚合模块里注册转发——这正是 `docs/layout-system-design.md §6` 规划的
//! 「4 类 Renderer」形态。

pub mod directed;
pub mod grid;
pub mod linear;
pub mod simple;

// 各图表的领域专属几何实现（由对应家族聚合模块转发调用）。
pub mod class;
pub mod er;
pub mod sequence;
pub mod timeline;
pub mod pie;
pub mod gitgraph;

use lievisual::scene::SceneNode;

use crate::ast::Diagram;
use crate::builder::layout::ir::PlacedGraph;
use crate::builder::layout::solver::{SolverKind, solver_for};
use crate::builder::types::OutputConfig;

pub use directed::DirectedRenderer;
pub use grid::GridRenderer;
pub use linear::LinearRenderer;
pub use simple::SimpleRenderer;

/// 渲染器统一入口：根据图表所属 solver 家族选渲染器，产出 `SceneNode` 列表。
///
/// `Directed` 家族的图表几何由 `PlacedGraph` 承载，因此传入 `placed`；
/// 其余家族（Grid/Linear/Simple）的几何是领域专属的，由各渲染器基于 AST 自绘，
/// 不经过通用 `PlacedGraph`。
pub fn render_placed(
    placed: &PlacedGraph,
    diagram: &Diagram,
    config: &OutputConfig,
) -> Vec<SceneNode> {
    match solver_for(diagram) {
        SolverKind::Directed => DirectedRenderer::render(placed, diagram, config),
        SolverKind::Grid => GridRenderer::render(diagram, config),
        SolverKind::Linear => LinearRenderer::render(diagram, config),
        SolverKind::Simple => SimpleRenderer::render(diagram, config),
    }
}
