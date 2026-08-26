//! `SimpleSolver`：pie / gitgraph 图的简单布局。

use lievisual::geometry::{Point, Size};

use super::super::config::LayoutConfig;
use super::super::ir::{LayoutGraph, PlacedGraph};

/// `SimpleSolver`：无节点排布（饼图）或简单线性（gitgraph 分支列）。
pub struct SimpleSolver;

impl SimpleSolver {
    pub fn solve(lg: &LayoutGraph, config: &LayoutConfig) -> PlacedGraph {
        // pie：无节点排布，仅标题（节点为空）。
        // gitgraph：按提交顺序线性排布。
        let mut positions = Vec::with_capacity(lg.nodes.len());
        let mut cur = 0.0;
        for node in &lg.nodes {
            positions.push(Point::new(cur + node.size.width / 2.0, node.size.height / 2.0));
            cur += node.size.width + config.node_gap;
        }
        PlacedGraph {
            positions,
            edge_routes: vec![],
            group_bounds: vec![],
            size: Size::new(0.0, 0.0),
        }
    }
}
