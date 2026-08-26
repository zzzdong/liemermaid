//! Grid 家族渲染器：把 `Class` / `Er` 归类到同一个聚合模块。
//!
//! class / er 的几何（节点位置、边路由）由 `GridSolver` 求解并写入 `PlacedGraph`，
//! `render_class` / `render_er` 消费该几何绘制类框/实体框与关系线，这里只做家族级转发，
//! 对应 `docs/layout-system-design.md §6` 的 `GridRenderer`。

use lievisual::scene::SceneNode;

use crate::ast::Diagram;
use crate::builder::layout::ir::PlacedGraph;
use crate::builder::types::OutputConfig;

use super::class;
use super::er;

/// Grid 家族（`Class` / `Er`）的统一渲染入口。
pub struct GridRenderer;

impl GridRenderer {
    pub fn render(placed: &PlacedGraph, diagram: &Diagram, config: &OutputConfig) -> Vec<SceneNode> {
        match diagram {
            Diagram::Class(cd) => class::render_class(placed, cd, config).unwrap_or_default(),
            Diagram::Er(ed) => er::render_er(placed, ed, config).unwrap_or_default(),
            _ => Vec::new(),
        }
    }
}
