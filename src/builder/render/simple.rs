//! Simple 家族渲染器：把 `Pie` / `GitGraph` 归类到同一个聚合模块。
//!
//! pie / gitgraph 的几何（扇形、提交 DAG）是领域专属的，由各自的 `render_pie` /
//! `render_gitgraph`（基于 AST 自绘）实现，这里只做家族级转发，
//! 对应 `docs/layout-system-design.md §6` 的 `SimpleRenderer`。

use lievisual::scene::SceneNode;

use crate::ast::Diagram;
use crate::builder::types::OutputConfig;

use super::gitgraph;
use super::pie;

/// Simple 家族（`Pie` / `GitGraph`）的统一渲染入口。
pub struct SimpleRenderer;

impl SimpleRenderer {
    pub fn render(diagram: &Diagram, config: &OutputConfig) -> Vec<SceneNode> {
        match diagram {
            Diagram::Pie(pd) => pie::render_pie(pd, config).unwrap_or_default(),
            Diagram::GitGraph(gd) => gitgraph::render_gitgraph(gd, config).unwrap_or_default(),
            _ => Vec::new(),
        }
    }
}
