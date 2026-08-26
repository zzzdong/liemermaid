//! 渲染层：把 `PlacedGraph`（纯几何）+ AST（形状/标签/箭头）渲染成 `SceneNode`。
//!
//! 渲染层只**读**坐标，不修改坐标。它拿 `PlacedGraph` 后回查 AST 取形状 / 文本 / 箭头，
//! 组合绘制。这是「求解」（改坐标）与「绘制」（读坐标）解耦的边界。

pub mod directed;

use lievisual::scene::SceneNode;

use crate::ast::Diagram;
use crate::builder::layout::ir::PlacedGraph;
use crate::builder::types::OutputConfig;

pub use directed::DirectedRenderer;

/// 渲染器统一入口：根据图表类型选渲染器，产出 `SceneNode` 列表。
pub fn render_placed(
    placed: &PlacedGraph,
    diagram: &Diagram,
    config: &OutputConfig,
) -> Vec<SceneNode> {
    match diagram {
        Diagram::Flowchart(fc) => DirectedRenderer::render_flowchart(placed, fc, config),
        Diagram::State(sd) => DirectedRenderer::render_state(placed, sd, config),
        _ => Vec::new(),
    }
}
