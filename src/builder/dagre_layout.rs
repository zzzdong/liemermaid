//! 用 dagre crate（dagrejs/dagre 的 Rust 移植）做 flowchart 分层布局。
//!
//! dagre 实现了完整的 Sugiyama 三阶段（acyclic FAS → network-simplex rank →
//! barycenter order → Brandes&Köpf coordinate assignment），与 mermaid 官方
//! 使用的布局引擎一致，从而消除自研管线带来的节点错位/重叠问题。
//!
//! 本模块直接消费 `Flowchart` AST，构造 dagre 图（含 subgraph 的 compound 模式），
//! 跑布局后返回「节点 id → 中心坐标」与「边 (源,目标) → 路径点」两组映射，
//! 由调用方构造 `SugiyamaResult`（无子图）或 `Layout` IR（有子图）复用现有渲染。

use std::collections::{HashMap, HashSet};

use dagre::graph::Graph as DagreGraph;
use dagre::graph::GraphOptions;
use dagre::{layout as dagre_layout_fn, EdgeLabel, LayoutOptions, NodeLabel, RankDir};
use lievisual::geometry::Point;

use crate::ast::{Direction, Flowchart, Subgraph};

/// dagre 布局结果（按节点 id 索引，便于调用方构造渲染 IR）
pub struct DagreLayout {
    /// 节点 id → 中心坐标（已是 `direction` 对应方向的绝对坐标）
    pub centers: HashMap<String, Point>,
    /// 边 (源 id, 目标 id) → 路径点
    pub edge_routes: HashMap<(String, String), Vec<Point>>,
}

/// 收集 flowchart 中所有"参与布局的节点 id"（顶层节点 + 各 subgraph 内部节点）
fn all_node_ids(fc: &Flowchart) -> Vec<String> {
    let mut ids: Vec<String> = fc.nodes.iter().map(|n| n.id.clone()).collect();
    for sg in &fc.subgraphs {
        for n in &sg.nodes {
            if !ids.contains(&n.id) {
                ids.push(n.id.clone());
            }
        }
    }
    ids
}

/// 用 dagre 对给定 flowchart 做布局。
///
/// - 无子图：普通有向图布局
/// - 有子图：启用 compound 模式，subgraph 作为容器节点，内部节点 `set_parent`
///   （dagre 的 nesting-graph 会据此调整 rank 与间距，与 mermaid 行为一致）
pub fn run_dagre(
    fc: &Flowchart,
    sizes: &HashMap<String, NodeSize>,
    direction: &Direction,
) -> DagreLayout {
    let (rankdir, swap) = match direction {
        Direction::TB | Direction::TD => (RankDir::TB, false),
        Direction::BT => (RankDir::BT, false),
        Direction::LR => (RankDir::LR, true),
        Direction::RL => (RankDir::RL, true),
    };

    let compound = !fc.subgraphs.is_empty();
    let mut dg: DagreGraph<NodeLabel, EdgeLabel> =
        DagreGraph::with_options(GraphOptions {
            compound,
            ..Default::default()
        });

    // 节点集（顶层 + subgraph 内部）
    let node_ids = all_node_ids(fc);
    for id in &node_ids {
        let nm = sizes.get(id).cloned().unwrap_or(NodeSize {
            width: 60.0,
            height: 30.0,
        });
        let (w, h) = if swap { (nm.height, nm.width) } else { (nm.width, nm.height) };
        dg.set_node(
            id.clone(),
            Some(NodeLabel {
                width: w,
                height: h,
                ..Default::default()
            }),
        );
    }

    // subgraph 容器节点（compound 模式需要）
    for sg in &fc.subgraphs {
        let sg_id = subgraph_node_id(sg);
        // 容器节点给一个保守尺寸，位置由成员包围盒决定（渲染时不使用其坐标）
        dg.set_node(
            sg_id.clone(),
            Some(NodeLabel {
                width: 1.0,
                height: 1.0,
                ..Default::default()
            }),
        );
        dg.set_parent(&sg_id, None);
        // 子图成员挂到容器下
        for n in &sg.nodes {
            dg.set_parent(&n.id, Some(&sg_id));
        }
    }

    // 边（顶层边 + subgraph 内部边）
    for edge in fc.edges.iter().chain(fc.subgraphs.iter().flat_map(|sg| sg.edges.iter())) {
        dg.set_edge(
            edge.source.clone(),
            edge.target.clone(),
            Some(EdgeLabel {
                minlen: 1,
                weight: 1,
                ..Default::default()
            }),
            None,
        );
    }

    // 与 mermaid 官方 run.js 对齐：nodesep=50, ranksep=60, ranker=network-simplex
    // （dagre crate 默认值虽也是 nodesep=50/ranksep=50，这里显式声明以消除歧义，
    // 并让 flowchart 的节点间隔明确可控）。
    let opts = LayoutOptions {
        rankdir,
        nodesep: 50.0,
        edgesep: 20.0,
        ranksep: 60.0,
        marginx: 8.0,
        marginy: 8.0,
        ranker: dagre::layout::types::Ranker::NetworkSimplex,
        ..Default::default()
    };
    dagre_layout_fn::layout(&mut dg, Some(opts));

    // 回填节点中心
    let mut centers = HashMap::new();
    for id in &node_ids {
        if let Some(l) = dg.node(id) {
            centers.insert(
                id.clone(),
                Point {
                    x: l.x.unwrap_or(0.0),
                    y: l.y.unwrap_or(0.0),
                },
            );
        }
    }
    for sg in &fc.subgraphs {
        let sg_id = subgraph_node_id(sg);
        if let Some(l) = dg.node(&sg_id) {
            centers.insert(
                sg_id,
                Point {
                    x: l.x.unwrap_or(0.0),
                    y: l.y.unwrap_or(0.0),
                },
            );
        }
    }

    // 回填边路径
    let mut edge_routes = HashMap::new();
    let all_edges: Vec<&crate::ast::Edge> =
        fc.edges.iter().chain(fc.subgraphs.iter().flat_map(|sg| sg.edges.iter())).collect();
    // 自环边（源 == 目标）：dagre 不支持自环，会给一条从中心绕图右侧回到中心的怪异曲线。
    // 这里跳过 dagre 的 route，改为在节点右侧（LR/RL 则下侧）画紧凑小环。
    let mut self_loop_routes: HashMap<(String, String), Vec<Point>> = HashMap::new();
    for edge in all_edges.iter() {
        if edge.source == edge.target {
            if let (Some(&c), Some(&s)) = (centers.get(&edge.source), sizes.get(&edge.source)) {
                let (hw, hh) = if swap { (s.height / 2.0, s.width / 2.0) } else { (s.width / 2.0, s.height / 2.0) };
                let route = if swap {
                    // LR/RL：节点竖放，自环从节点底部画出 U 形（起止于底边左右，向下凸出），
                    // 避开向右走的出边，避免重叠。起止点在底边界而非中心。
                    let bottom = c.y + hh;
                    let dx = hw * 0.28;
                    let loop_h = (hw * 1.4).max(18.0);
                    vec![
                        Point::new(c.x - dx, bottom),
                        Point::new(c.x - dx, bottom + loop_h * 0.5),
                        Point::new(c.x, bottom + loop_h),
                        Point::new(c.x + dx, bottom + loop_h * 0.5),
                        Point::new(c.x + dx, bottom),
                    ]
                } else {
                    // TB/BT：节点横放，自环从节点右侧画出 U 形（起止于右边缘上下，向右凸出），
                    // 避开向下走的出边（如 B->C），避免重叠。起止点在右边界而非中心。
                    let right = c.x + hw;
                    let dy = hh * 0.28;
                    let loop_w = (hw * 0.9).max(16.0);
                    vec![
                        Point::new(right, c.y - dy),
                        Point::new(right + loop_w, c.y - dy),
                        Point::new(right + loop_w, c.y + dy),
                        Point::new(right, c.y + dy),
                    ]
                };
                self_loop_routes.insert((edge.source.clone(), edge.target.clone()), route);
            }
        }
    }

    for edge in all_edges {
        // 自环边：直接用预生成的小环 route，跳过 dagre 查找
        if let Some(route) = self_loop_routes.get(&(edge.source.clone(), edge.target.clone())) {
            edge_routes.insert((edge.source.clone(), edge.target.clone()), route.clone());
            continue;
        }

        // dagre 内部对平行反向边（如 B->C 与 C->B）会按无向键去重，只保留一个方向。
        // 因此精确方向查找可能失败；此时反向查找并反转点序，复用同一条折线。
        let mut found = dg.edge(&edge.source, &edge.target, None);
        let mut reversed = false;
        if found.is_none() {
            found = dg.edge(&edge.target, &edge.source, None);
            reversed = true;
        }
        if let Some(el) = found {
            if !el.points.is_empty() {
                let mut route: Vec<Point> = el.points.iter().map(|p| Point { x: p.x, y: p.y }).collect();
                if reversed {
                    route.reverse();
                }
                edge_routes.insert((edge.source.clone(), edge.target.clone()), route);
            }
        }
    }

    // dagre 对无向平行边（含双向边 B->C 与 C->B）只生成一条折线，
    // 导致两条有向边视觉上重叠成"单线双向"。这里对互相反向的边对做
    // 中间点偏移，使两条线从同一对节点边界出发、中段平行分离，
    // 贴近 mermaid 官方对双向边的渲染。
    separate_parallel_edges(fc, &mut edge_routes, &centers, &sizes);

    // 菱形节点的边端点需要从矩形边界裁剪到菱形实际边缘，
    // 否则连线会"悬空"在菱形外部（dagre 按矩形包围盒算端点）。
    clip_diamond_endpoints(fc, &mut edge_routes, &centers, &sizes);

    DagreLayout {
        centers,
        edge_routes,
    }
}

/// subgraph 在 dagre 中的容器节点 id（避免与业务节点 id 冲突，加前缀）
fn subgraph_node_id(sg: &Subgraph) -> String {
    format!("__subgraph__{}", sg.title.clone().unwrap_or_default())
}

/// 把图中互相反向的边对（如 B->C 与 C->B）的折线路由做平行分离。
///
/// dagre（Rust crate）对无向平行的边只生成一条折线，liemermaid 回填时
/// 两条有向边复用了同一份点序（一条正向、一条反向反转），视觉上成为
/// "单线双向箭头"。本函数对每对反向边，仅偏移其中间控制点（保留首尾端点
/// 把图中互相反向的边对（如 B->C 与 C->B）的折线路由做平行分离。
///
/// dagre（Rust crate）对无向平行的边只生成一条折线，liemermaid 回填时
/// 两条有向边复用了同一份点序（一条正向、一条反向反转），视觉上成为
/// "单线双向箭头"。本函数对每对反向边，沿连线法向把**整条线**（含端点）
/// 偏移 ±sep/2，并把两端点吸附回各自节点边界（沿"指向对端"方向的交点），
/// 使两条线严格平行、间距 sep，且端点从节点边界的不同位置出发，
/// 贴近 mermaid 官方对双向边的渲染。
fn separate_parallel_edges(
    fc: &Flowchart,
    routes: &mut HashMap<(String, String), Vec<Point>>,
    centers: &HashMap<String, Point>,
    sizes: &HashMap<String, NodeSize>,
) {
    let sep = 10.0;
    let edges: Vec<&crate::ast::Edge> = fc
        .edges
        .iter()
        .chain(fc.subgraphs.iter().flat_map(|sg| sg.edges.iter()))
        .collect();
    let mut done: HashSet<(String, String)> = HashSet::new();
    for e in &edges {
        let key = (e.source.clone(), e.target.clone());
        if done.contains(&key) {
            continue;
        }
        // 自环边（源 == 目标）由 run_dagre 预生成小环，且 separate_one 的端点吸附
        // 会把首尾吸到节点中心（toward 取自身中心），故这里跳过，保留预生成的小环。
        if e.source == e.target {
            done.insert(key);
            continue;
        }
        let Some(rev_e) = edges
            .iter()
            .find(|o| o.source == e.target && o.target == e.source)
        else {
            continue;
        };
        // 以正向边方向定义连线法向（固定），两条边沿该反向分别 ±sep/2 偏移，
        // 保证严格平行分离（反向边自身方向求法向会相互抵消，故用法向固定）。
        let (nx, ny) = {
            let (Some(ca), Some(cb)) = (centers.get(&e.source), centers.get(&e.target)) else {
                continue;
            };
            let dx = cb.x - ca.x;
            let dy = cb.y - ca.y;
            let l = (dx * dx + dy * dy).sqrt();
            if l > 1e-9 {
                (-dy / l, dx / l)
            } else {
                (0.0, 0.0)
            }
        };
        let rev_key = (rev_e.source.clone(), rev_e.target.clone());
        separate_one(routes.get_mut(&key), sep / 2.0, nx, ny, centers, sizes, &e.source, &e.target);
        separate_one(
            routes.get_mut(&rev_key),
            -sep / 2.0,
            nx,
            ny,
            centers,
            sizes,
            &rev_e.source,
            &rev_e.target,
        );
        done.insert(key);
        done.insert(rev_key);
    }
}

/// 对单条反向边：让连线向法向外侧鼓起，端点吸附回节点边界。
///
/// 官方 mermaid 的双向边不是简单平移，而是两根对称向外弯曲的曲线。
/// 对只有 2 个端点的相邻层边，插入两个鼓包控制点；对已有中间点的长边，
/// 用 taper 函数让中间点比端点偏移得更远，形成向外的弧线。
fn separate_one(
    route: Option<&mut Vec<Point>>,
    delta: f64,
    nx: f64,
    ny: f64,
    centers: &HashMap<String, Point>,
    sizes: &HashMap<String, NodeSize>,
    src: &str,
    dst: &str,
) {
    let Some(r) = route else {
        return;
    };
    let n = r.len();
    if n < 2 {
        return;
    }

    // 先把端点吸附到节点边界（沿指向对端方向），并加上法向偏移
    if let (Some(c), Some(s)) = (centers.get(src), sizes.get(src)) {
        let base = boundary_point(c, s.width / 2.0, s.height / 2.0, centers.get(dst));
        r[0] = Point::new(base.x + nx * delta, base.y + ny * delta);
    }
    if let (Some(c), Some(s)) = (centers.get(dst), sizes.get(dst)) {
        let base = boundary_point(c, s.width / 2.0, s.height / 2.0, centers.get(src));
        r[n - 1] = Point::new(base.x + nx * delta, base.y + ny * delta);
    }

    if n == 2 {
        // 相邻层直线边：插入两个鼓包控制点，使连线向外弯曲。
        let p0 = r[0];
        let p1 = r[1];
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let l = (dx * dx + dy * dy).sqrt();
        if l > 1e-9 {
            let dvx = dx / l;
            let dvy = dy / l;
            let step = l * 0.25;
            let bump = (l * 0.20).max(8.0); // 鼓起幅度：间距 20%，最小 8px
            let q1 = Point::new(
                p0.x + dvx * step + nx * bump,
                p0.y + dvy * step + ny * bump,
            );
            let q2 = Point::new(
                p1.x - dvx * step + nx * bump,
                p1.y - dvy * step + ny * bump,
            );
            *r = vec![p0, q1, q2, p1];
        }
    } else {
        // 已有中间点的长边：中间点比端点偏移得更远，形成 taper 鼓包。
        let total = (n - 1) as f64;
        for i in 1..n - 1 {
            let t = (i as f64) / total; // 0..1
            let envelope = (t * std::f64::consts::PI).sin(); // 两端 0，中间 1
            let k = 1.0 + envelope; // 端点处 1，中间处 2
            r[i].x += nx * delta * k;
            r[i].y += ny * delta * k;
        }
    }
}

/// 从节点中心沿 `toward` 方向射线与节点矩形边界的交点（端点吸附用）。
fn boundary_point(c: &Point, half_w: f64, half_h: f64, toward: Option<&Point>) -> Point {
    let Some(t) = toward else {
        return *c;
    };
    let dx = t.x - c.x;
    let dy = t.y - c.y;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return *c;
    }
    let sx = if dx.abs() < 1e-9 {
        f64::INFINITY
    } else {
        half_w / dx.abs()
    };
    let sy = if dy.abs() < 1e-9 {
        f64::INFINITY
    } else {
        half_h / dy.abs()
    };
    let s = sx.min(sy);
    Point::new(c.x + dx * s, c.y + dy * s)
}

/// 从菱形中心沿 `toward` 方向射线与菱形边缘的交点。
///
/// 菱形方程：|x - cx| / w + |y - cy| / h = 1（w=半宽, h=半高）。
/// 给定方向 d = normalize(toward - center)，交点 = center - d * t，
/// 其中 t = 1 / (|d.x|/w + |d.y|/h)。
fn diamond_boundary_point(c: &Point, half_w: f64, half_h: f64, toward: &Point) -> Point {
    let dx = toward.x - c.x;
    let dy = toward.y - c.y;
    let l = (dx * dx + dy * dy).sqrt();
    if l < 1e-9 {
        // toward 与中心重合，返回菱形顶点（上方）
        return Point::new(c.x, c.y - half_h);
    }
    let ux = dx / l; // 单位方向向量
    let uy = dy / l;
    let denom = ux.abs() / half_w + uy.abs() / half_h;
    if denom < 1e-9 {
        return *c;
    }
    let t = 1.0 / denom;
    Point::new(c.x + ux * t, c.y + uy * t)
}

/// 遍历所有边，对源或目标为 Diamond 形状的节点，
/// 将边端点从矩形边界裁剪到菱形实际边缘。
fn clip_diamond_endpoints(
    fc: &Flowchart,
    routes: &mut HashMap<(String, String), Vec<Point>>,
    centers: &HashMap<String, Point>,
    sizes: &HashMap<String, NodeSize>,
) {
    use crate::ast::NodeShape;

    // 构建 id → shape 映射
    let mut shape_map: HashMap<String, NodeShape> = HashMap::new();
    for n in &fc.nodes {
        shape_map.insert(n.id.clone(), n.shape.clone().unwrap_or(NodeShape::Rectangle));
    }
    for sg in &fc.subgraphs {
        for n in &sg.nodes {
            shape_map.insert(n.id.clone(), n.shape.clone().unwrap_or(NodeShape::Rectangle));
        }
    }

    for (key, pts) in routes.iter_mut() {
        if pts.len() < 2 {
            continue;
        }
        // 裁剪起点（源节点为 Diamond）
        if shape_map.get(&key.0) == Some(&NodeShape::Diamond) {
            if let (Some(c), Some(s)) = (centers.get(&key.0), sizes.get(&key.0)) {
                let target = if pts.len() == 2 { &pts[1] } else { &pts[1] };
                pts[0] = diamond_boundary_point(c, s.width / 2.0, s.height / 2.0, target);
            }
        }
        // 裁剪终点（目标节点为 Diamond）
        if shape_map.get(&key.1) == Some(&NodeShape::Diamond) {
            if let (Some(c), Some(s)) = (centers.get(&key.1), sizes.get(&key.1)) {
                let last = pts.len() - 1;
                let src_idx = last.saturating_sub(1);
                let src_pt = pts[src_idx];
                pts[last] = diamond_boundary_point(c, s.width / 2.0, s.height / 2.0, &src_pt);
            }
        }
    }
}

/// 节点尺寸（dagre 用）
#[derive(Debug, Clone, Copy)]
pub struct NodeSize {
    pub width: f64,
    pub height: f64,
}
