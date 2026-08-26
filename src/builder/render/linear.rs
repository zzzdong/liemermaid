//! Linear 家族渲染器：把 `Sequence` / `Timeline` 归类到同一个聚合模块。
//!
//! sequence / timeline 的几何（生命线 + 激活条、时间轴分栏）是领域专属的，由各自的
//! `render_sequence` / `render_timeline`（基于 AST 自绘）实现，这里只做家族级转发，
//! 对应 `docs/layout-system-design.md §6` 的 `LinearRenderer`。

use lievisual::scene::SceneNode;

use crate::ast::Diagram;
use crate::builder::types::OutputConfig;

use super::sequence;
use super::timeline;

/// Linear 家族（`Sequence` / `Timeline`）的统一渲染入口。
pub struct LinearRenderer;

impl LinearRenderer {
    pub fn render(diagram: &Diagram, config: &OutputConfig) -> Vec<SceneNode> {
        match diagram {
            Diagram::Sequence(sd) => sequence::render_sequence(sd, config),
            Diagram::Timeline(td) => timeline::render_timeline(td, config),
            _ => Vec::new(),
        }
    }
}
