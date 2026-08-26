//! `GridSolver`：class / er 图的行列网格布局。

use lievisual::geometry::{Point, Size};

use super::super::config::LayoutConfig;
use super::super::ir::{LayoutGraph, PlacedGraph};

/// `GridSolver`：节点按行填充的网格布局。
pub struct GridSolver;

impl GridSolver {
    pub fn solve(lg: &LayoutGraph, config: &LayoutConfig) -> PlacedGraph {
        let mut positions = Vec::with_capacity(lg.nodes.len());
        let mut max_row_h = 0.0;
        let mut cur_x = 0.0;
        let mut cur_y = 0.0;
        // 每行最多放的节点数（简单启发式：按画布宽度自适应，此处固定 4 列，后续可调）
        let max_per_row = 4;

        for (i, node) in lg.nodes.iter().enumerate() {
            if i > 0 && i % max_per_row == 0 {
                // 换行
                cur_x = 0.0;
                cur_y += max_row_h + config.layer_gap;
                max_row_h = 0.0;
            }
            positions.push(Point::new(
                cur_x + node.size.width / 2.0,
                cur_y + node.size.height / 2.0,
            ));
            cur_x += node.size.width + config.node_gap;
            max_row_h = max_row_h.max(node.size.height);
        }

        // 边：简单直线（源中心 → 目标中心）
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
