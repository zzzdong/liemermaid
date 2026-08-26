//! Grid 家族渲染器：把 `Class` / `Er` 归类到同一个聚合模块。
//!
//! class / er 的几何（类框分栏、实体框 + 关系路由）是领域专属的，由各自的
//! `render_class` / `render_er`（基于 AST 自绘）实现，这里只做家族级转发，
//! 对应 `docs/layout-system-design.md §6` 的 `GridRenderer`。

use lievisual::scene::SceneNode;

use crate::ast::Diagram;
use crate::builder::types::OutputConfig;

use super::class;
use super::er;

/// Grid 家族（`Class` / `Er`）的统一渲染入口。
pub struct GridRenderer;

impl GridRenderer {
    pub fn render(diagram: &Diagram, config: &OutputConfig) -> Vec<SceneNode> {
        match diagram {
            Diagram::Class(cd) => class::render_class(cd, config).unwrap_or_default(),
            Diagram::Er(ed) => er::render_er(ed, config).unwrap_or_default(),
            _ => Vec::new(),
        }
    }
}
