use std::collections::HashMap;

use lievisual::text::{RichSpan, layout_text};

use crate::{
    ast::{Node, NodeShape},
    builder::types::OutputConfig,
    vir::{Color, TextAlign, TextBaseline, TextStyle},
};

use super::{
    coord::NodeAnchors,
    types::{NodeId, NodeMetrics, Size},
};
use crate::builder::theme;

// 测量与绘制共用 theme.rs 的同一套配置（字体、节点尺寸、padding），
// 保证 dagre 分配的节点包围盒与实际绘制矩形完全一致。
const MIN_NODE_WIDTH: f64 = theme::NODE_MIN_W;
const MIN_NODE_HEIGHT: f64 = theme::NODE_MIN_H;
const NODE_PAD_X: f64 = theme::NODE_PAD_X;
const NODE_PAD_Y: f64 = theme::NODE_PAD_Y;
const FONT_SIZE: f64 = theme::FONT_SIZE;
const FONT_FAMILY: &str = theme::FONT_FAMILY;

fn shape_multiplier(shape: &Option<NodeShape>) -> (f64, f64, f64, f64) {
    match shape {
        Some(NodeShape::Diamond) => (
            MIN_NODE_WIDTH * 1.4,
            MIN_NODE_HEIGHT * 1.4,
            NODE_PAD_X * 1.6,
            NODE_PAD_Y * 1.6,
        ),
        Some(NodeShape::Hexagon) => (
            MIN_NODE_WIDTH * 1.3,
            MIN_NODE_HEIGHT * 1.2,
            NODE_PAD_X * 1.4,
            NODE_PAD_Y * 1.2,
        ),
        Some(NodeShape::Circle) | Some(NodeShape::DoubleCircle) => (
            MIN_NODE_HEIGHT * 1.3,
            MIN_NODE_HEIGHT * 1.3,
            NODE_PAD_X * 1.3,
            NODE_PAD_Y * 1.3,
        ),
        Some(NodeShape::Cylinder) => (
            MIN_NODE_WIDTH * 1.1,
            MIN_NODE_HEIGHT * 1.2,
            NODE_PAD_X * 1.2,
            NODE_PAD_Y * 1.5,
        ),
        Some(NodeShape::Stadium) => (
            MIN_NODE_WIDTH * 1.1,
            MIN_NODE_HEIGHT,
            NODE_PAD_X * 1.5,
            NODE_PAD_Y,
        ),
        Some(NodeShape::Asymmetric) => (
            MIN_NODE_WIDTH * 1.1,
            MIN_NODE_HEIGHT,
            NODE_PAD_X * 1.3,
            NODE_PAD_Y,
        ),
        Some(NodeShape::Parallelogram) | Some(NodeShape::ParallelogramAlt) => (
            MIN_NODE_WIDTH * 1.2,
            MIN_NODE_HEIGHT,
            NODE_PAD_X * 1.5,
            NODE_PAD_Y,
        ),
        Some(NodeShape::Trapezoid) | Some(NodeShape::TrapezoidAlt) => (
            MIN_NODE_WIDTH * 1.2,
            MIN_NODE_HEIGHT,
            NODE_PAD_X * 1.4,
            NODE_PAD_Y,
        ),
        _ => (MIN_NODE_WIDTH, MIN_NODE_HEIGHT, NODE_PAD_X, NODE_PAD_Y),
    }
}

/// Pass 2: 测量所有节点的尺寸
pub fn measure_nodes(nodes: &[Node], config: &OutputConfig) -> HashMap<NodeId, NodeMetrics> {
    let mut metrics = HashMap::new();

    for node in nodes {
        let m = measure_node(node, config);
        metrics.insert(node.id.clone(), m);
    }

    metrics
}

fn measure_node(node: &Node, _config: &OutputConfig) -> NodeMetrics {
    let text = node.text.as_deref().unwrap_or(&node.id);

    let text_style = TextStyle::new(Color::BLACK, FONT_SIZE, FONT_FAMILY)
        .with_align(TextAlign::Center)
        .with_baseline(TextBaseline::Middle);

    let layout = layout_text(&[RichSpan::new(text.to_string(), text_style.clone())], None);
    let text_w = layout.width;
    let text_h = layout.height;

    // 节点尺寸由文字排版结果决定；不同形状有各自几何约束，
    // 必须在测量阶段落实（而非绘制阶段补救），以保证布局锚点正确。
    let size = match &node.shape {
        Some(NodeShape::Circle) | Some(NodeShape::DoubleCircle) => {
            // 圆/双圆：强制正方形，直径 = 文字最大维度 + 左右留白
            let pad = NODE_PAD_X;
            let d = (text_w.max(text_h) + 2.0 * pad).max(MIN_NODE_HEIGHT);
            Size::new(d, d)
        }
        Some(NodeShape::Stadium) => {
            // Stadium（跑道形）：两端半圆直径 = 高，故宽 = 文字宽 + 高（两端半圆各占半高），
            // 横向仅留极小内边距，避免文字短时节点过宽。
            let h = (text_h + 2.0 * NODE_PAD_Y).max(MIN_NODE_HEIGHT);
            let r = h / 2.0;
            let w = (text_w + 2.0 * r + 8.0).max(MIN_NODE_WIDTH);
            Size::new(w, h)
        }
        Some(NodeShape::Cylinder) => {
            // 圆柱：顶部椭圆额外占用一段高度
            let ellipse = NODE_PAD_Y * 2.0;
            let h = (text_h + 2.0 * NODE_PAD_Y + ellipse).max(MIN_NODE_HEIGHT);
            let w = (text_w + 2.0 * NODE_PAD_X).max(MIN_NODE_WIDTH);
            Size::new(w, h)
        }
        _ => {
            let (min_w, min_h, pad_x, pad_y) = shape_multiplier(&node.shape);
            Size::new(
                min_w.max(text_w + 2.0 * pad_x),
                min_h.max(text_h + 2.0 * pad_y),
            )
        }
    };

    let anchors = NodeAnchors::new((size.width, size.height));

    NodeMetrics { size, anchors }
}
