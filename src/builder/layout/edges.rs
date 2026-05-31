use std::collections::{HashMap, HashSet};

use vello_cpu::kurbo::Point;

use crate::ast::{Direction, Edge};

use super::types::{NodeId, NodePosition, RoutedEdge};

pub fn route_edges(
    edges: &[Edge],
    positions: &HashMap<NodeId, NodePosition>,
    _layers: &HashMap<NodeId, usize>,
    back_edge_pairs: &HashSet<(NodeId, NodeId)>,
    direction: Direction,
) -> Vec<RoutedEdge> {
    edges
        .iter()
        .map(|edge| {
            let is_back = back_edge_pairs.contains(&(edge.source.clone(), edge.target.clone()));
            route_single_edge(edge, positions, is_back, &direction)
        })
        .collect()
}

fn route_single_edge(
    edge: &Edge,
    positions: &HashMap<NodeId, NodePosition>,
    is_back: bool,
    direction: &Direction,
) -> RoutedEdge {
    let from_pos = positions.get(&edge.source);
    let to_pos = positions.get(&edge.target);

    let is_horizontal = *direction == Direction::LR || *direction == Direction::RL;

    let route = match (from_pos, to_pos) {
        (Some(from), Some(to)) => {
            if is_back {
                route_back_edge(from, to)
            } else if *direction == Direction::BT {
                route_bt(from, to)
            } else if is_horizontal {
                route_horizontal(from, to, *direction == Direction::RL)
            } else {
                let from_bottom = from.center.y + from.anchors.bottom.y;
                let to_top = to.center.y + to.anchors.top.y;
                if from_bottom > to_top + 1.0 {
                    route_same_level(from, to)
                } else {
                    route_vertical(from, to)
                }
            }
        }
        _ => vec![],
    };

    RoutedEdge {
        edge: edge.clone(),
        route,
        label_position: None,
    }
}

fn route_vertical(from: &NodePosition, to: &NodePosition) -> Vec<Point> {
    let from_bottom = Point::new(from.center.x, from.center.y + from.anchors.bottom.y);
    let to_top = Point::new(to.center.x, to.center.y + to.anchors.top.y);
    let mid_y = (from_bottom.y + to_top.y) / 2.0;

    vec![
        from_bottom,
        Point::new(from_bottom.x, mid_y),
        Point::new(to_top.x, mid_y),
        to_top,
    ]
}

/// BT 方向：源在下方，从源顶部连接到目标底部（方向反转）
fn route_bt(from: &NodePosition, to: &NodePosition) -> Vec<Point> {
    let from_top = Point::new(from.center.x, from.center.y + from.anchors.top.y);
    let to_bottom = Point::new(to.center.x, to.center.y + to.anchors.bottom.y);
    let mid_y = (from_top.y + to_bottom.y) / 2.0;

    vec![
        from_top,
        Point::new(from_top.x, mid_y),
        Point::new(to_bottom.x, mid_y),
        to_bottom,
    ]
}

fn route_horizontal(from: &NodePosition, to: &NodePosition, is_rl: bool) -> Vec<Point> {
    if is_rl {
        // RL 方向：源在右侧，从源左侧连接到目标右侧
        let from_left = Point::new(from.center.x + from.anchors.left.x, from.center.y);
        let to_right = Point::new(to.center.x + to.anchors.right.x, to.center.y);
        let mid_x = (from_left.x + to_right.x) / 2.0;

        vec![
            from_left,
            Point::new(mid_x, from_left.y),
            Point::new(mid_x, to_right.y),
            to_right,
        ]
    } else {
        let from_right = Point::new(from.center.x + from.anchors.right.x, from.center.y);
        let to_left = Point::new(to.center.x + to.anchors.left.x, to.center.y);
        let mid_x = (from_right.x + to_left.x) / 2.0;

        vec![
            from_right,
            Point::new(mid_x, from_right.y),
            Point::new(mid_x, to_left.y),
            to_left,
        ]
    }
}

fn route_back_edge(from: &NodePosition, to: &NodePosition) -> Vec<Point> {
    let from_top = Point::new(from.center.x, from.center.y + from.anchors.top.y);
    let to_top = Point::new(to.center.x, to.center.y + to.anchors.top.y);
    let rise = 20.0;

    vec![
        from_top,
        Point::new(from_top.x, from_top.y - rise),
        Point::new(to_top.x, from_top.y - rise),
        to_top,
    ]
}

/// 同一层级的节点间路由：从侧面连接到侧面
fn route_same_level(from: &NodePosition, to: &NodePosition) -> Vec<Point> {
    let to_left_of_from = to.center.x < from.center.x;

    let (from_anchor, to_anchor) = if to_left_of_from {
        (
            Point::new(from.center.x + from.anchors.left.x, from.center.y),
            Point::new(to.center.x + to.anchors.right.x, to.center.y),
        )
    } else {
        (
            Point::new(from.center.x + from.anchors.right.x, from.center.y),
            Point::new(to.center.x + to.anchors.left.x, to.center.y),
        )
    };

    let mid_x = (from_anchor.x + to_anchor.x) / 2.0;

    vec![
        from_anchor,
        Point::new(mid_x, from_anchor.y),
        Point::new(mid_x, to_anchor.y),
        to_anchor,
    ]
}