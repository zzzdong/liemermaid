//! `LinearSolver`：sequence / timeline 图的线性（时间轴）布局。

use lievisual::geometry::{Point, Size};

use super::super::config::LayoutConfig;
use super::super::ir::{LayoutGraph, PlacedGraph};

/// `LinearSolver`：节点沿主轴线性排布。
pub struct LinearSolver;

impl LinearSolver {
    pub fn solve(lg: &LayoutGraph, config: &LayoutConfig) -> PlacedGraph {
        let mut positions = Vec::with_capacity(lg.nodes.len());
        let mut cur = 0.0;
        for node in &lg.nodes {
            positions.push(Point::new(cur + node.size.width / 2.0, node.size.height / 2.0));
            cur += node.size.width + config.node_gap;
        }
        let edge_routes: Vec<Vec<Point>> = lg
            .edges
            .iter()
            .map(|e| {
                if e.source < positions.len() && e.target < positions.len() {
                    vec![positions[e.source], positions[e.target]]
                } else {
                    vec![]
                }
            })
            .collect();
        PlacedGraph {
            positions,
            edge_routes,
            group_bounds: vec![],
            size: Size::new(0.0, 0.0),
        }
    }
}
