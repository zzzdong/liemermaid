//! LayoutEngine —— 布局求解入口（P1.1 Sugiyama 分层 + 交叉减少）。
//!
//! 编排：`extract` 产出的 UG → `directed::sugiyama_layers`（分层 + 降交叉 + 方向反转）
//! → 按 `direction` 做主轴坐标分配 → 端口解析 → 直线边路由（P1.2 升级为正交路由）。
//!
//! 产出 `(Geograph, StyleIntent)`：几何与视觉意图在 layout 结束时分离，
//! 使 UG 可在此后 drop，materialize / paint 不持有 UG 引用。

use std::collections::HashMap;

use lievisual::geometry::Point;

use crate::ast::Direction;
use crate::builder::ir::{
    self,
    common::*,
    geograph::{GGContainer, GGEdge, GGNode, Geograph},
    shape::ShapeKind,
    unigraph::EdgeKind,
    StyleIntent,
};
use crate::builder::layout::directed::sugiyama_layers;
use crate::builder::layout::route::route_edges;
use crate::builder::theme;

const LAYER_GAP: f64 = 40.0;
const NODE_GAP: f64 = 40.0;

/// 把 UG 布局成 GG，并抽取 StyleIntent。
pub fn run(ug: &ir::Unigraph) -> Result<(Geograph, StyleIntent), String> {
    // 索引节点尺寸（measure 已写入 MeasuredLabel）
    let mut sizes: HashMap<NodeId, Size> = HashMap::new();
    let mut shapes: HashMap<NodeId, ShapeKind> = HashMap::new();
    let mut labels: HashMap<NodeId, ir::common::MeasuredLabel> = HashMap::new();
    for n in &ug.nodes {
        let size = match &n.label {
            LabelOrMeasured::Measured(m) => m.size,
            _ => Size::new(theme::NODE_MIN_W, theme::NODE_MIN_H),
        };
        sizes.insert(n.id.clone(), size);
        shapes.insert(n.id.clone(), n.shape);
        if let LabelOrMeasured::Measured(m) = &n.label {
            labels.insert(n.id.clone(), m.clone());
        }
    }

    // —— 分层（Sugiyama：最长路径 + 交叉减少 + 方向反转）——
    let layers = sugiyama_layers(ug);
    let n_layers = layers.len();

    // —— 坐标分配（主轴随 direction 旋转）——
    // 主轴 = 层推进方向；同层轴 = 同层节点展开方向。
    let horizontal_main = matches!(ug.direction, Direction::LR | Direction::RL);

    // 每层「主尺寸」（层内节点在「同层轴」上的累计长度）与「跨尺寸」（层在主轴方向的厚度）
    let mut layer_main_len = vec![0.0f64; n_layers];
    let mut layer_cross_thick = vec![0.0f64; n_layers];
    for (li, layer) in layers.iter().enumerate() {
        let mut main_len = 0.0f64;
        let mut max_cross = 0.0f64;
        for id in layer {
            let size = sizes.get(id).cloned().unwrap_or(Size::new(theme::NODE_MIN_W, theme::NODE_MIN_H));
            let (cross, main) = if horizontal_main {
                (size.height, size.width)
            } else {
                (size.width, size.height)
            };
            main_len += main + NODE_GAP;
            max_cross = max_cross.max(cross);
        }
        main_len = main_len.max(NODE_GAP) - NODE_GAP;
        layer_main_len[li] = main_len;
        layer_cross_thick[li] = max_cross;
    }

    // 主轴起点（居中）：总主轴长度
    let total_main: f64 = layer_main_len.iter().sum::<f64>() + LAYER_GAP * (n_layers as f64 - 1.0).max(0.0);
    let mut main_cursor = -total_main / 2.0;

    // 各层主轴坐标（层中心在主轴上的位置）
    let mut layer_main_coord = vec![0.0f64; n_layers];
    for li in 0..n_layers {
        let cross_thick = layer_cross_thick[li];
        layer_main_coord[li] = main_cursor + cross_thick / 2.0;
        main_cursor += cross_thick + LAYER_GAP;
    }

    // 同层轴坐标分配（居中展开）
    let mut centers: HashMap<NodeId, Point> = HashMap::new();
    for (li, layer) in layers.iter().enumerate() {
        let main_len = layer_main_len[li];
        let mut cross_cursor = -main_len / 2.0;
        for id in layer {
            let size = sizes.get(id).cloned().unwrap_or(Size::new(theme::NODE_MIN_W, theme::NODE_MIN_H));
            let main = if horizontal_main {
                size.width
            } else {
                size.height
            };
            let half_main = main / 2.0;
            let main_coord = layer_main_coord[li];
            let cross_coord = cross_cursor + half_main;
            let (x, y) = if horizontal_main {
                (main_coord, cross_coord)
            } else {
                (cross_coord, main_coord)
            };
            centers.insert(id.clone(), Point::new(x, y));
            cross_cursor += main + NODE_GAP;
        }
    }

    // —— 构建 GGNode（含端口）——
    let mut gg_nodes = Vec::new();
    for n in &ug.nodes {
        let center = centers.get(&n.id).cloned().unwrap_or(Point::new(0.0, 0.0));
        let size = sizes.get(&n.id).cloned().unwrap_or(Size::new(theme::NODE_MIN_W, theme::NODE_MIN_H));
        let shape = shapes.get(&n.id).cloned().unwrap_or(ShapeKind::Rectangle);
        gg_nodes.push(GGNode {
            id: n.id.clone(),
            role: n.role,
            center,
            size,
            shape,
            ports: resolve_ports(center, size),
            label: labels.get(&n.id).cloned(),
        });
    }

    // —— 边路由（P1.1 直线：source 端口 → target 端口）——
    let mut gg_edges = Vec::new();
    let node_lookup: HashMap<&NodeId, &GGNode> = gg_nodes.iter().map(|n| (&n.id, n)).collect();
    for e in &ug.edges {
        let (Some(s), Some(t)) = (node_lookup.get(&e.source), node_lookup.get(&e.target)) else {
            continue;
        };
        let start = port_point(s, e.source_port);
        let end = port_point(t, e.target_port);
        gg_edges.push(GGEdge {
            id: e.id.clone(),
            source: e.source.clone(),
            target: e.target.clone(),
            route: vec![start, end],
            label_anchor: None,
            kind: EdgeKind::Flow,
            arrow: e.arrow,
            routing_hint: e.routing_hint,
        });
    }

    // —— StyleIntent 抽取 ——
    let style = StyleIntent {
        node_styles: ug
            .nodes
            .iter()
            .map(|n| (n.id.clone(), n.style_ref.clone()))
            .collect(),
        edge_styles: ug
            .edges
            .iter()
            .map(|e| (e.id.clone(), EdgeKind::Flow, e.arrow))
            .collect(),
        container_styles: Vec::new(),
    };

    // 正交边路由（节点回避）：先放入 GG，再统一重路由
    let mut gg = Geograph {
        size: Size::new(0.0, 0.0),
        background: theme::BACKGROUND,
        nodes: gg_nodes,
        edges: gg_edges,
        containers: Vec::<GGContainer>::new(),
    };
    route_edges(&mut gg);

    let bbox = compute_bbox(&gg.nodes, &gg.edges);
    gg.size = bbox;
    Ok((gg, style))
}

/// 据节点 center + size 解析四向端口坐标。
fn resolve_ports(center: Point, size: Size) -> ir::common::ResolvedPorts {
    ir::common::ResolvedPorts {
        top: Point::new(center.x, center.y - size.height / 2.0),
        bottom: Point::new(center.x, center.y + size.height / 2.0),
        left: Point::new(center.x - size.width / 2.0, center.y),
        right: Point::new(center.x + size.width / 2.0, center.y),
    }
}

/// 取某端口的实际坐标。
fn port_point(node: &GGNode, hint: PortHint) -> Point {
    match hint {
        PortHint::Top => node.ports.top,
        PortHint::Bottom => node.ports.bottom,
        PortHint::Left => node.ports.left,
        PortHint::Right => node.ports.right,
        PortHint::Auto => node.ports.bottom,
    }
}

/// 计算所有节点 + 边点的包围盒尺寸（留白边距）。
fn compute_bbox(nodes: &[GGNode], edges: &[GGEdge]) -> Size {
    let mut min_x = 0.0f64;
    let mut min_y = 0.0f64;
    let mut max_x = 0.0f64;
    let mut max_y = 0.0f64;
    for n in nodes {
        min_x = min_x.min(n.center.x - n.size.width / 2.0);
        min_y = min_y.min(n.center.y - n.size.height / 2.0);
        max_x = max_x.max(n.center.x + n.size.width / 2.0);
        max_y = max_y.max(n.center.y + n.size.height / 2.0);
    }
    for e in edges {
        for p in &e.route {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
    }
    let pad = 20.0f64;
    Size::new(max_x - min_x + pad * 2.0, max_y - min_y + pad * 2.0)
}
