//! `EdgeRouter`：边路由 + 节点回避。
//!
//! 输入 [`crate::builder::ir::geograph::Geograph`]，原地更新每条边的 `route`。
//! 路由策略按边类型分派：
//!
//! 1. **自环**（source == target）：节点右侧外凸环（单段贝塞尔）。
//! 2. **双向对**（u↔v 同时存在）：小偏移 + 轻微外弓的平行曲线（官方风格）。
//! 3. **回边**（source 在主轴下游）：中间无阻挡 → 侧向 U 形贝塞尔；
//!    有阻挡 → 侧边通道绕行，避免长回边斜穿中间层节点。
//! 4. **普通边**：Spline → 单段贝塞尔，穿过节点时退化为「stub + 侧边通道 +
//!    圆角折线」绕行；Orthogonal → 曼哈顿折线，主干步进搜索无冲突位置。
//!
//! 端口分散：同一节点同一端口的多条边按对端投影排序后沿节点边错开；
//! 端口点对 Diamond / Circle 形状做**边界投影**（端口永远落在形状边界上）。
//!
//! **容器避障**：`route_edges` 额外接收容器框（`GGContainer.bounds`）作为障碍，
//! 边只绕开「两端均非其成员」的容器；容器内边 / 跨容器边（至少一端为成员）
//! 允许正常穿过容器边界。

use std::collections::{HashMap, HashSet};

use lievisual::geometry::{Point, Rect};

use crate::builder::ir::common::RoutingHint;
use crate::builder::ir::geograph::{GGContainer, GGNode, Geograph, RoutePath, RouteSegment};
use crate::builder::ir::shape::ShapeKind;

/// 出线 stub 长度（端口法向延伸距离）。
const STUB: f64 = 18.0;
/// 端口槽位：该边在节点对应端口边（Top/Bottom/Left/Right）上的序号与总数。
#[derive(Clone, Copy)]
pub(crate) struct Slot {
    pub(crate) idx: usize,
    pub(crate) total: usize,
}
/// 样条控制点沿端口法向延伸的上下限。
const MIN_ARC: f64 = 28.0;
const MAX_ARC: f64 = 140.0;
/// 端口分散：相邻槽位的最小间距（节点很窄 / 边很多时的下限）。
const PORT_MIN_SPACING: f64 = 14.0;
/// 端口分散：相邻槽位间距上限（相对该条边的长度）。
const PORT_MAX_SPACING_RATIO: f64 = 0.4;
/// 端口分散：端口点距节点角的最小留白。
const PORT_CORNER_MARGIN: f64 = 8.0;
/// 节点回避偏移步进。
const OFFSET_STEP: f64 = 12.0;
/// 回避最大尝试次数。
const MAX_OFFSET_TRIES: usize = 8;
/// 通道与节点包围盒的间距。
const CHANNEL_MARGIN: f64 = 16.0;
/// 通道堆叠间距：多条边的通道重叠时，沿侧向逐格错开。
const CH_STACK: f64 = 28.0;
/// 回边阻挡检测带的外扩（沿同层轴）。
const BAND_PAD: f64 = 30.0;
/// 折线圆角半径（通道绕行路径的平滑度）。
const CORNER_RADIUS: f64 = 14.0;
/// 容器框避让 padding：容器包围盒外扩，防止边贴边框。
const CONTAINER_PAD: f64 = 6.0;

/// 每条边的路由计划（分类结果）。
#[derive(Clone, Copy)]
enum Plan {
    /// 自环（source == target）。
    SelfLoop,
    /// 双向对（u↔v）：`side` = +1 右侧 / -1 左侧。
    Mutual { side: f64 },
    /// 回边（无阻挡）：侧向 U 形贝塞尔。
    BackBow { side: f64 },
    /// 回边（有阻挡）：沿 `ch`（主轴法向坐标，垂直空间）的通道绕行。
    BackChannel { ch: f64, side: f64 },
    /// 普通边。
    Normal,
}

/// 避让矩形集合：节点 + 容器。
///
/// 容器避让规则：边只绕开「两端均非其成员」的容器；
/// 若边至少一端是某容器成员，则该边允许穿过该容器（容器内边 / 跨容器边）。
struct Obstacles {
    /// 全部避让矩形（owner 为节点 id 或容器 id），已含 padding。
    rects: Vec<(usize, Rect, String)>,
    /// 容器 id → 成员节点 id 集合（用于跳过「本容器内部 / 跨本容器的边」）。
    members: HashMap<String, HashSet<String>>,
}

impl Obstacles {
    /// 转置到垂直空间（水平主轴时统一在垂直空间规划）。
    fn transposed(&self) -> Obstacles {
        Obstacles {
            rects: self
                .rects
                .iter()
                .map(|(i, r, o)| (*i, tr_rect(*r), o.clone()))
                .collect(),
            members: self.members.clone(),
        }
    }
}

/// 为一条边构造「是否跳过某避让矩形」的判定：
/// - 端点节点自身不避让；
/// - 边至少一端是某容器成员时，该容器不避让（容器内边 / 跨容器边）。
fn edge_skip<'a>(
    s_id: &'a str,
    t_id: &'a str,
    members: &'a HashMap<String, HashSet<String>>,
) -> impl Fn(&str) -> bool + 'a {
    move |owner: &str| -> bool {
        if owner == s_id || owner == t_id {
            return true;
        }
        members
            .get(owner)
            .is_some_and(|m| m.contains(s_id) || m.contains(t_id))
    }
}

/// 为 GG 中所有边生成路由。
///
/// `containers`：子图容器框。边只绕开「两端均非其成员」的容器，
/// 容器内边 / 跨容器边（至少一端为成员）允许穿过容器边界。
pub fn route_edges(
    gg: &mut Geograph,
    direction: crate::ast::Direction,
    containers: &[GGContainer],
) {
    let node_map: HashMap<&String, &GGNode> = gg.nodes.iter().map(|n| (&n.id, n)).collect();

    // 节点包围盒（含少量 padding）。
    let node_rects: Vec<(usize, Rect, String)> = gg
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (i, node_rect(n), n.id.clone()))
        .collect();

    // 容器包围盒（外扩 CONTAINER_PAD）与成员映射：参与边避让。
    let container_rects: Vec<(usize, Rect, String)> = containers
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let b = c.bounds;
            (
                i,
                Rect::new(
                    b.min_x() - CONTAINER_PAD,
                    b.min_y() - CONTAINER_PAD,
                    b.max_x() + CONTAINER_PAD,
                    b.max_y() + CONTAINER_PAD,
                ),
                c.id.clone(),
            )
        })
        .collect();
    let mut members: HashMap<String, HashSet<String>> = HashMap::new();
    for c in containers {
        members.insert(c.id.clone(), c.member_ids.iter().cloned().collect());
    }
    let mut all_rects = node_rects.clone();
    all_rects.extend(container_rects);
    let obstacles = Obstacles {
        rects: all_rects,
        members,
    };

    // mutual 对（u↔v）的绕行侧分配记录：key = (source, target)，value = ±1。
    let mut mutual_side: HashMap<(String, String), f64> = HashMap::new();

    // 已占用的绕行通道（垂直空间）：(ch, y0, y1)。边间避让：后规划的边
    // 不得与已占用通道同坐标且跨度重叠，否则两条边画在同一条线上。
    let mut used_channels: Vec<(f64, f64, f64)> = Vec::new();
    // 已占用的正交主干（trunk 坐标, 跨度起, 跨度止）：同层间的并行正交边
    // 主干互斥，避免多条边画在同一条主干线上（阶段 0.6）。
    let mut used_trunks: Vec<(f64, f64, f64)> = Vec::new();
    let edge_pairs: Vec<(String, String)> = gg
        .edges
        .iter()
        .map(|e| (e.source.clone(), e.target.clone()))
        .collect();

    // 预计算 mutual 对集合（u↔v 与 v↔u 同时存在）。
    let mut mutual_pairs: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    for e in &gg.edges {
        if gg
            .edges
            .iter()
            .any(|o| o.source == e.target && o.target == e.source)
        {
            mutual_pairs.insert((e.source.clone(), e.target.clone()));
        }
    }

    // —— 分类：为每条边生成路由计划 ——
    let mut plans: Vec<Option<Plan>> = Vec::with_capacity(gg.edges.len());
    for e in &gg.edges {
        match (node_map.get(&e.source), node_map.get(&e.target)) {
            (Some(s), Some(t)) => {
                let plan = if s.id == t.id {
                    Plan::SelfLoop
                } else if mutual_pairs.contains(&(e.source.clone(), e.target.clone())) {
                    Plan::Mutual {
                        side: mutual_side_for(&e.source, &e.target, &mut mutual_side),
                    }
                } else if is_back_edge(s, t, direction) {
                    let side = mutual_side_for(&e.source, &e.target, &mut mutual_side);
                    // 所有回边（含 Orthogonal）都先尝试侧通道绕行：有阻挡 → 通道
                    // （绕开中间节点），无阻挡 → U 形贝塞尔（本来就安全）。
                    // 旧实现仅对 Spline 尝试通道，Orthogonal 回边直接 BackBow，
                    // 长回边会斜穿中间层节点。
                    match plan_back_channel(
                        s,
                        t,
                        direction,
                        &node_rects,
                        &obstacles,
                        &edge_pairs,
                        &mut used_channels,
                    ) {
                        Some((ch, ch_side)) => Plan::BackChannel { ch, side: ch_side },
                        None => Plan::BackBow { side },
                    }
                } else {
                    Plan::Normal
                };
                plans.push(Some(plan));
            }
            _ => plans.push(None),
        }
    }

    // —— 端口分配 ——
    // 自环 / 双向对使用专用路由，不参与槽位分组。
    let mut edge_ports: Vec<Option<(Port, Port)>> = Vec::with_capacity(gg.edges.len());
    for (i, e) in gg.edges.iter().enumerate() {
        let pair = (node_map.get(&e.source), node_map.get(&e.target));
        let ports = match plans[i] {
            Some(Plan::SelfLoop) | Some(Plan::Mutual { .. }) => None,
            Some(Plan::BackBow { .. }) => match pair {
                (Some(s), Some(t)) => Some(ports_for(s, t, direction, true)),
                _ => None,
            },
            Some(Plan::BackChannel { side, .. }) => Some(channel_ports(side, direction)),
            Some(Plan::Normal) | None => match pair {
                (Some(s), Some(t)) => Some(ports_for(s, t, direction, false)),
                _ => None,
            },
        };
        edge_ports.push(ports);
    }

    // 分组：key = (节点 id, 端口)，value = [(边索引, 对端在该端口边轴向上的投影)]
    let mut src_groups: HashMap<(String, Port), Vec<(usize, f64)>> = HashMap::new();
    let mut tgt_groups: HashMap<(String, Port), Vec<(usize, f64)>> = HashMap::new();
    for (i, e) in gg.edges.iter().enumerate() {
        let Some((sp, tp)) = edge_ports[i] else {
            continue;
        };
        let (Some(s), Some(t)) = (node_map.get(&e.source), node_map.get(&e.target)) else {
            continue;
        };
        src_groups
            .entry((e.source.clone(), sp))
            .or_default()
            .push((i, axis_projection(sp, t.center)));
        tgt_groups
            .entry((e.target.clone(), tp))
            .or_default()
            .push((i, axis_projection(tp, s.center)));
    }

    // 组内按对端投影排序 → 槽位 (idx, total)：
    // 端口排列顺序与对端的空间顺序一致，fan-out/fan-in 不会互相错位打结。
    let mut slots: Vec<(Slot, Slot)> =
        vec![(Slot { idx: 0, total: 1 }, Slot { idx: 0, total: 1 }); gg.edges.len()];
    rank_slots(src_groups, &mut slots, true);
    rank_slots(tgt_groups, &mut slots, false);

    // —— 逐边生成路由 ——
    for (i, e) in gg.edges.iter_mut().enumerate() {
        let Some(plan) = plans[i] else {
            continue;
        };
        let (Some(s), Some(t)) = (node_map.get(&e.source), node_map.get(&e.target)) else {
            continue;
        };
        let s = *s;
        let t = *t;
        let (s_slot, t_slot) = slots[i];

        let mut anchor_override = None;
        // 自环 / 双向对使用专用路由（无端口概念），其余边需要端口配对。
        let route = match plan {
            Plan::SelfLoop => {
                let (r, anchor) = self_loop_route(s);
                anchor_override = Some(anchor);
                r
            }
            Plan::Mutual { side } => mutual_dual_route(s, t, side),
            _ => {
                let Some((sp, tp)) = edge_ports[i] else {
                    continue;
                };
                let s_port = (sp, s_slot);
                let t_port = (tp, t_slot);
                match plan {
                    Plan::BackBow { side } => back_edge_route(s, t, &s_port, &t_port, side),
                    Plan::BackChannel { ch, .. } => channel_route(s, t, ch, &s_port, &t_port),
                    _ => match e.routing_hint {
                        RoutingHint::Spline => spline_route_safe(
                            s,
                            t,
                            &s_port,
                            &t_port,
                            &obstacles,
                            &mut used_channels,
                        ),
                        _ => orthogonal_route(s, t, &s_port, &t_port, &obstacles, &mut used_trunks),
                    },
                }
            }
        };
        // 边标签锚点：取路由中段的中点；自环取环外侧。
        if e.label_text.is_some() {
            e.label_anchor = Some(anchor_override.unwrap_or_else(|| route.midpoint()));
        }
        e.route = route;
    }
}

/// 检测 mutual 对并为每条边分配绕行侧。
/// `side`: +1 = 右侧，-1 = 左侧。两条相对边一左一右对称。
fn mutual_side_for(source: &str, target: &str, seen: &mut HashMap<(String, String), f64>) -> f64 {
    let key = (source.to_owned(), target.to_owned());
    if let Some(&s) = seen.get(&key) {
        return s;
    }
    let reverse_key = (target.to_owned(), source.to_owned());
    // 若反向边已分配，则本条分配相反侧。
    if let Some(&rev) = seen.get(&reverse_key) {
        let side = -rev;
        seen.insert(key, side);
        side
    } else {
        seen.insert(key, 1.0);
        1.0
    }
}

// ============================================================
// 自环
// ============================================================

/// 自环路由：从节点右侧伸出一个小环，绕回节点右侧（官方 mermaid 风格）。
/// 返回（路由，环外侧的标签锚点）。
fn self_loop_route(n: &GGNode) -> (RoutePath, Point) {
    let (hw, hh) = (n.size.width / 2.0, n.size.height / 2.0);
    // 起止点取在形状边界上（菱形/圆按边界投影）。
    let p0 = port_point_offset(n, Port::Right, -hh * 0.3);
    let p3 = port_point_offset(n, Port::Right, hh * 0.3);
    let ext = (hh * 1.2).max(30.0);
    let outer = n.center.x + hw + ext;
    let p1 = Point::new(outer, n.center.y - hh * 0.9);
    let p2 = Point::new(outer, n.center.y + hh * 0.9);
    let anchor = Point::new(outer + 10.0, n.center.y);

    let mut r = RoutePath::new();
    r.push(RouteSegment::CubicBezier { p0, p1, p2, p3 });
    (r, anchor)
}

// ============================================================
// 双向对（u↔v）
// ============================================================

/// 双向对路由：两条相对边小偏移 + 轻微外弓（官方 mermaid 的"双线紧贴"风格）。
///
/// 偏移量约为节点宽度的 7%（clamp 4~12px），两条线一左一右几乎平行，
/// 控制点沿同侧再外弓一点，形成柔和的弧线而非僵硬直线。
fn mutual_dual_route(s: &GGNode, t: &GGNode, side: f64) -> RoutePath {
    let dx = (t.center.x - s.center.x).abs();
    let dy = (t.center.y - s.center.y).abs();
    let off = (s.size.width.min(t.size.width) * 0.07).clamp(4.0, 12.0);
    let bow = off * 0.9;

    let mut r = RoutePath::new();
    if dy >= dx {
        // 垂直堆叠：上节点底边出、下节点顶边入（反向边反之）。
        let forward = t.center.y >= s.center.y;
        let sp = if forward { Port::Bottom } else { Port::Top };
        let tp = if forward { Port::Top } else { Port::Bottom };
        let p0 = port_point_offset(s, sp, side * off);
        let p3 = port_point_offset(t, tp, side * off);
        let h = p3.y - p0.y;
        let p1 = Point::new(p0.x + side * bow, p0.y + h * 0.45);
        let p2 = Point::new(p3.x + side * bow, p3.y - h * 0.45);
        r.push(RouteSegment::CubicBezier { p0, p1, p2, p3 });
    } else {
        // 水平堆叠：左节点右边出、右节点左边入。
        let forward = t.center.x >= s.center.x;
        let sp = if forward { Port::Right } else { Port::Left };
        let tp = if forward { Port::Left } else { Port::Right };
        let p0 = port_point_offset(s, sp, side * off);
        let p3 = port_point_offset(t, tp, side * off);
        let w = p3.x - p0.x;
        let p1 = Point::new(p0.x + w * 0.45, p0.y + side * bow);
        let p2 = Point::new(p3.x - w * 0.45, p3.y + side * bow);
        r.push(RouteSegment::CubicBezier { p0, p1, p2, p3 });
    }
    r
}

// ============================================================
// 回边
// ============================================================

/// 回边路由（无阻挡时的侧向 U 形贝塞尔）：源节点主轴反向端口出，
/// 目标节点对侧端口入，控制点沿主轴外扩，形成侧向绕行的弧线。
fn back_edge_route(
    s: &GGNode,
    t: &GGNode,
    s_slot: &(Port, Slot),
    t_slot: &(Port, Slot),
    side: f64,
) -> RoutePath {
    let (sp, tp) = (s_slot.0, t_slot.0);
    let p0 = port_point_at(s, sp, s_slot.1);
    let p3 = port_point_at(t, tp, t_slot.1);

    let mut r = RoutePath::new();
    if matches!(sp, Port::Top | Port::Bottom) {
        // 垂直主轴：控制点沿 y 外扩、x 按 side 错开。
        let d = if p3.y >= p0.y { 1.0 } else { -1.0 };
        let y_ext = (p3.y - p0.y).abs() * 0.4 + 20.0;
        let x_off = s.size.width.max(t.size.width) * 0.25;
        let p1 = Point::new(p0.x + side * x_off, p0.y + d * y_ext);
        let p2 = Point::new(p3.x - side * x_off, p3.y - d * y_ext);
        r.push(RouteSegment::CubicBezier { p0, p1, p2, p3 });
    } else {
        // 水平主轴：控制点沿 x 外扩、y 按 side 错开。
        let d = if p3.x >= p0.x { 1.0 } else { -1.0 };
        let x_ext = (p3.x - p0.x).abs() * 0.4 + 20.0;
        let y_off = s.size.height.max(t.size.height) * 0.25;
        let p1 = Point::new(p0.x + d * x_ext, p0.y + side * y_off);
        let p2 = Point::new(p3.x - d * x_ext, p3.y - side * y_off);
        r.push(RouteSegment::CubicBezier { p0, p1, p2, p3 });
    }
    r
}

/// 回边通道绕行路由：从源/目标的侧边端口出发，经垂直（或水平）通道直连，
/// 圆角折线平滑过渡。
fn channel_route(
    s: &GGNode,
    t: &GGNode,
    ch: f64,
    s_slot: &(Port, Slot),
    t_slot: &(Port, Slot),
) -> RoutePath {
    let (sp, tp) = (s_slot.0, t_slot.0);
    let p0 = port_point_at(s, sp, s_slot.1);
    let p3 = port_point_at(t, tp, t_slot.1);
    // 侧端口（Left/Right）→ 垂直通道，直接在真实空间构建；
    // 顶/底端口 → 水平通道，转置到垂直空间构建后转置回来。
    let vertical = matches!(sp, Port::Left | Port::Right);
    let tr = |p: Point| if vertical { p } else { Point::new(p.y, p.x) };
    let (a, b) = (tr(p0), tr(p3));
    let pts_v = [a, Point::new(ch, a.y), Point::new(ch, b.y), b];
    if vertical {
        rounded_polyline(&pts_v, CORNER_RADIUS)
    } else {
        let pts: Vec<Point> = pts_v.iter().map(|p| tr(*p)).collect();
        rounded_polyline(&pts, CORNER_RADIUS)
    }
}

/// 通道是否与已占用通道同坐标且跨度重叠。
fn channel_conflicts_used(ch: f64, top: f64, bot: f64, used: &[(f64, f64, f64)]) -> bool {
    used.iter()
        .any(|(c, y0, y1)| (c - ch).abs() < 2.0 && *y1 > top && *y0 < bot)
}

/// 正交主干是否与已占用主干同坐标且跨度重叠。
fn trunk_conflicts_used(trunk: f64, s0: f64, s1: f64, used: &[(f64, f64, f64)]) -> bool {
    used.iter()
        .any(|(t, u0, u1)| (t - trunk).abs() < 2.0 && *u1 > s0 && *u0 < s1)
}

/// 尝试某一侧的通道：先让开节点/容器，再避开已占用通道（冲突时沿侧向堆叠一格重试）。
#[allow(clippy::too_many_arguments)]
fn try_channel_side(
    start: f64,
    to_right: bool,
    top: f64,
    bot: f64,
    rects: &[(usize, Rect, String)],
    skip: &dyn Fn(&str) -> bool,
    ends_clear: &dyn Fn(f64) -> bool,
    used: &[(f64, f64, f64)],
) -> Option<f64> {
    let mut ch = settle_channel(start, to_right, top, bot, rects, skip)?;
    if !ends_clear(ch) {
        return None;
    }
    for _ in 0..6 {
        if !channel_conflicts_used(ch, top, bot, used) {
            return Some(ch);
        }
        // 沿远离通道的方向堆叠一格，并重新让开节点/容器。
        ch = if to_right {
            ch + CH_STACK
        } else {
            ch - CH_STACK
        };
        ch = settle_channel(ch, to_right, top, bot, rects, skip)?;
        if !ends_clear(ch) {
            return None;
        }
    }
    None
}

/// 规划回边通道：若 source/target 之间存在节点或容器阻挡，计算一条无冲突且
/// 不与其他边通道重叠的侧边通道坐标（垂直空间）。
/// 成功时登记占用并返回（通道坐标, 侧向 ±1）；无阻挡或无解 → None。
fn plan_back_channel(
    s: &GGNode,
    t: &GGNode,
    direction: crate::ast::Direction,
    node_rects: &[(usize, Rect, String)],
    obstacles: &Obstacles,
    edges: &[(String, String)],
    used: &mut Vec<(f64, f64, f64)>,
) -> Option<(f64, f64)> {
    use crate::ast::Direction;
    let vert = matches!(direction, Direction::TB | Direction::TD | Direction::BT);
    let tr = |p: Point| if vert { p } else { Point::new(p.y, p.x) };
    let sc = tr(s.center);
    let tc = tr(t.center);
    // 回边：源在 target 下游（垂直空间 sc.y > tc.y）。异常朝向放弃通道。
    if sc.y <= tc.y {
        return None;
    }
    let (top, bot) = (tc.y, sc.y);
    let mid_x = (sc.x + tc.x) / 2.0;
    // 垂直主轴保持原矩形；水平主轴转置到垂直空间。
    let rects_v: Vec<(usize, Rect, String)> = if vert {
        obstacles.rects.to_vec()
    } else {
        obstacles.transposed().rects
    };
    // 仅节点矩形（转置后）供 crosser 计数取节点中心。
    let node_rects_v: Vec<(usize, Rect, String)> = if vert {
        node_rects.to_vec()
    } else {
        node_rects
            .iter()
            .map(|(i, r, o)| (*i, tr_rect(*r), o.clone()))
            .collect()
    };
    let skip = edge_skip(&s.id, &t.id, &obstacles.members);

    // 阻挡带：两中心之间的矩形（外扩 BAND_PAD），命中即视为阻挡。
    let band = Rect::new(
        (sc.x.min(tc.x) - BAND_PAD).min(mid_x - BAND_PAD),
        top,
        (sc.x.max(tc.x) + BAND_PAD).max(mid_x + BAND_PAD),
        bot,
    );
    let mut any = false;
    let mut ob_min = f64::INFINITY;
    let mut ob_max = f64::NEG_INFINITY;
    for (_, r, owner) in &rects_v {
        if skip(owner) {
            continue;
        }
        if rects_overlap(*r, band) {
            any = true;
            ob_min = ob_min.min(r.min_x());
            ob_max = ob_max.max(r.max_x());
        }
    }
    if !any {
        return None;
    }

    // 左右两侧各推一条通道，并验证水平引入段（源/目标中心高度处）。
    let ends_clear = |ch: f64| -> bool {
        [sc.y, tc.y].iter().all(|&y| {
            polyline_clear(
                &[Point::new(sc.x.min(ch), y), Point::new(sc.x.max(ch), y)],
                &skip,
                &rects_v,
            ) && polyline_clear(
                &[Point::new(tc.x.min(ch), y), Point::new(tc.x.max(ch), y)],
                &skip,
                &rects_v,
            )
        })
    };
    let right = try_channel_side(
        ob_max + CHANNEL_MARGIN,
        true,
        top,
        bot,
        &rects_v,
        &skip,
        &ends_clear,
        used,
    );
    let left = try_channel_side(
        ob_min - CHANNEL_MARGIN,
        false,
        top,
        bot,
        &rects_v,
        &skip,
        &ends_clear,
        used,
    );

    // 交叉者计数：候选通道之外（beyond）若有节点，且该节点有边连到
    // 「通道跨度内的节点」（如 E→C 连 C，C 位于 B..D 跨度内），则该边的
    // 绕行/进入段会与本通道交叉 —— 该侧应降级。用于在两侧等距时选空侧。
    let center_of = |id: &str| -> Option<(f64, f64)> {
        node_rects_v
            .iter()
            .find(|(_, _, o)| o == id)
            .map(|(_, r, _)| ((r.min_x() + r.max_x()) / 2.0, (r.min_y() + r.max_y()) / 2.0))
    };
    let crossers = |ch: f64, to_right: bool| -> usize {
        let beyond = |x: f64| if to_right { x > ch } else { x < ch };
        let mut cnt = 0usize;
        for (u, v) in edges {
            if u == &s.id || u == &t.id || v == &s.id || v == &t.id {
                continue;
            }
            let (Some(uc), Some(vc)) = (center_of(u), center_of(v)) else {
                continue;
            };
            let in_span = if beyond(uc.0) {
                vc.1
            } else if beyond(vc.0) {
                uc.1
            } else {
                continue;
            };
            if in_span > top && in_span < bot {
                cnt += 1;
            }
        }
        cnt
    };
    let score =
        |ch: f64, to_right: bool| -> (usize, f64) { (crossers(ch, to_right), (ch - mid_x).abs()) };

    let chosen = match (right, left) {
        (Some(r), Some(l)) => {
            if score(r, true) <= score(l, false) {
                Some((r, 1.0))
            } else {
                Some((l, -1.0))
            }
        }
        (Some(r), None) => Some((r, 1.0)),
        (None, Some(l)) => Some((l, -1.0)),
        (None, None) => None,
    };
    if let Some((ch, _)) = chosen {
        used.push((ch, top, bot));
    }
    chosen
}

/// 从初始通道坐标出发，沿 `to_right` 方向逐个让开挡路的节点/容器矩形，
/// 直到垂直段 (ch, top..bot) 无冲突。超过迭代上限返回 None。
fn settle_channel(
    mut ch: f64,
    to_right: bool,
    top: f64,
    bot: f64,
    rects: &[(usize, Rect, String)],
    skip: &dyn Fn(&str) -> bool,
) -> Option<f64> {
    for _ in 0..60 {
        let mut blocker: Option<Rect> = None;
        for (_, r, owner) in rects {
            if skip(owner) {
                continue;
            }
            if r.min_y() <= bot && r.max_y() >= top && r.min_x() < ch && r.max_x() > ch {
                blocker = Some(*r);
                break;
            }
        }
        let Some(r) = blocker else { return Some(ch) };
        ch = if to_right {
            r.max_x() + CHANNEL_MARGIN
        } else {
            r.min_x() - CHANNEL_MARGIN
        };
    }
    None
}

// ============================================================
// 普通边：Spline
// ============================================================

/// Spline 路由：单段三次贝塞尔（官方 mermaid 风格）。
///
/// 端口锚点（含槽位分散）作为 P0/P3；控制点 P1/P2 沿源/目标端口法向延伸，
/// 并沿**垂直流向**（横向）偏置，产生平滑的扇形弧线。
///
/// 关键约束（保证 fan-out / fan-in 多条边**互不交叉**）：
/// - 法向延伸量 `arc` 只取**两端口沿流向的净间距** `gap` 的一半，绝不超过 `gap`。
///   若按 `max(|dx|,|dy|)` 取弧度，`gap` 很小而横向跨度很大的边（典型 fan-out）
///   会把控制点甩过目标行的外侧，形成 S 形回绕 —— 相邻出边因而互相穿过。
/// - `arc <= gap` 时曲线在主轴上是**单调**的，同一源节点的多条出边不会相交。
fn spline_route(s: &GGNode, t: &GGNode, s_slot: &(Port, Slot), t_slot: &(Port, Slot)) -> RoutePath {
    let sp = s_slot.0;
    let tp = t_slot.0;
    let p0 = port_point_at(s, sp, s_slot.1);
    let p3 = port_point_at(t, tp, t_slot.1);

    // 端口出/入方向：沿端口法向（离开节点）。
    let (ox, oy) = port_normal(sp);
    let (ix, iy) = port_normal(tp);

    // 沿流向（source 端口法向）的净间距 = 两端口之间可用的"空白走廊"长度。
    // 负值（逆流，如非 back-edge 的回绕情形）按 0 处理。
    let gap = ((p3.x - p0.x) * ox + (p3.y - p0.y) * oy).max(0.0);

    // 弧度：取间距的一半并 clamp；再约束 `arc <= gap`，避免控制点越过对端造成回绕。
    // `gap == 0`（同层同高）时退回 MIN_ARC，让边略微鼓出而不至于退化成直线。
    let mut arc = (gap * 0.5).clamp(MIN_ARC, MAX_ARC);
    if gap > 0.0 {
        arc = arc.min(gap);
    }

    // 横向偏置：取垂直于流向的位移分量（纵向端口 → 横向；横向端口 → 纵向），
    // 让 fan-out / fan-in 的多条边呈对称扇形展开。系数 1/4 保证 x 控制点逐项单调
    // （P0 → P0+1/4 → P3-1/4 → P3），曲线在横向也单调。
    let vx = p3.x - p0.x;
    let vy = p3.y - p0.y;
    let tvx = vx * (1.0 - ox * ox) - vy * ox * oy;
    let tvy = vy * (1.0 - oy * oy) - vx * ox * oy;
    let bias = 0.25;

    let p1 = Point::new(p0.x + ox * arc + tvx * bias, p0.y + oy * arc + tvy * bias);
    let p2 = Point::new(p3.x + ix * arc - tvx * bias, p3.y + iy * arc - tvy * bias);

    let mut r = RoutePath::new();
    r.push(RouteSegment::CubicBezier { p0, p1, p2, p3 });
    r
}

/// 带节点/容器回避的 Spline 路由：先按朴素贝塞尔生成并采样检测；
/// 若穿过非端点节点或「两端均非成员」的容器，则改为「stub + 侧边通道 +
/// 圆角折线」的绕行路径（官方 mermaid 的长边风格：出节点后平移到旁侧空通道，
/// 直行，再平滑进入目标）。
fn spline_route_safe(
    s: &GGNode,
    t: &GGNode,
    s_slot: &(Port, Slot),
    t_slot: &(Port, Slot),
    obstacles: &Obstacles,
    used_channels: &mut Vec<(f64, f64, f64)>,
) -> RoutePath {
    let naive = spline_route(s, t, s_slot, t_slot);
    let (sp, tp) = (s_slot.0, t_slot.0);
    let vertical = matches!(sp, Port::Top | Port::Bottom);
    let tr = |p: Point| if vertical { p } else { Point::new(p.y, p.x) };
    let skip = edge_skip(&s.id, &t.id, &obstacles.members);

    // 统一在垂直空间做检测与规划（水平主轴时转置，垂直主轴保持原样）。
    let rects_v: Vec<(usize, Rect, String)> = if vertical {
        obstacles.rects.to_vec()
    } else {
        obstacles.transposed().rects
    };

    // 采样朴素曲线，收集穿过的非端点节点 / 非成员容器。
    let samples_v: Vec<Point> = sample_path(&naive, 32).iter().map(|p| tr(*p)).collect();
    let mut hit = false;
    let mut hit_min = f64::INFINITY;
    let mut hit_max = f64::NEG_INFINITY;
    for w in samples_v.windows(2) {
        for (_, r, owner) in &rects_v {
            if skip(owner) {
                continue;
            }
            if segment_intersects_rect(w[0], w[1], r) {
                hit = true;
                hit_min = hit_min.min(r.min_x());
                hit_max = hit_max.max(r.max_x());
            }
        }
    }
    if !hit {
        return naive;
    }

    // 绕行折线（垂直空间）：p0 → 出 stub → 平移到通道 → 直行 → 入 stub → p3。
    let p0v = tr(port_point_at(s, sp, s_slot.1));
    let p3v = tr(port_point_at(t, tp, t_slot.1));
    let down = p3v.y >= p0v.y;
    let av = Point::new(p0v.x, p0v.y + if down { STUB } else { -STUB });
    let bv = Point::new(p3v.x, p3v.y + if down { -STUB } else { STUB });
    let top = av.y.min(bv.y);
    let bot = av.y.max(bv.y);

    // 候选通道：命中障碍两侧 + 所有障碍最外侧（兜底）。
    let mut candidates: Vec<f64> = Vec::new();
    if let Some(ch) = settle_channel(hit_max + CHANNEL_MARGIN, true, top, bot, &rects_v, &skip) {
        candidates.push(ch);
    }
    if let Some(ch) = settle_channel(hit_min - CHANNEL_MARGIN, false, top, bot, &rects_v, &skip) {
        candidates.push(ch);
    }
    let all_max = rects_v
        .iter()
        .map(|(_, r, _)| r.max_x())
        .fold(f64::NEG_INFINITY, f64::max);
    let all_min = rects_v
        .iter()
        .map(|(_, r, _)| r.min_x())
        .fold(f64::INFINITY, f64::min);
    candidates.push(all_max + CHANNEL_MARGIN);
    candidates.push(all_min - CHANNEL_MARGIN);

    // 两轮评估：先要求不与已占用通道重叠（边间避让），
    // 全部被拒时放宽（节点/容器避让优先于边间避让）。
    let mut best: Option<(f64, f64)> = None; // (cost, ch)
    for strict in [true, false] {
        for ch in &candidates {
            let pts = [
                p0v,
                av,
                Point::new(*ch, av.y),
                Point::new(*ch, bv.y),
                bv,
                p3v,
            ];
            if !polyline_clear(&pts, &skip, &rects_v) {
                continue;
            }
            if strict && channel_conflicts_used(*ch, top, bot, used_channels) {
                continue;
            }
            let cost = (ch - p0v.x).abs() + (ch - p3v.x).abs();
            let better = match best {
                None => true,
                Some((bc, bch)) => cost < bc || (cost == bc && *ch > bch),
            };
            if better {
                best = Some((cost, *ch));
            }
        }
        if best.is_some() {
            break;
        }
    }
    let Some((_, ch)) = best else { return naive };
    used_channels.push((ch, top, bot));

    let pts_v = [p0v, av, Point::new(ch, av.y), Point::new(ch, bv.y), bv, p3v];
    if vertical {
        rounded_polyline(&pts_v, CORNER_RADIUS)
    } else {
        let pts: Vec<Point> = pts_v.iter().map(|p| tr(*p)).collect();
        rounded_polyline(&pts, CORNER_RADIUS)
    }
}

// ============================================================
// 普通边：Orthogonal
// ============================================================

/// 正交路由：stub + 主干折线（曼哈顿）。
///
/// 主干位置（垂直主轴 = 共享 x；水平主轴 = 共享 y）从两 stub 端点中点出发，
/// 沿主轴法向双向步进搜索，取「冲突最少、偏移最小」的落点，
/// 处理长边跨层（主干初始位置落在中间层节点内）的情况。
///
/// 阶段 0.6（边-边避让）：正交主干互斥 —— 后规划的边不与已占用主干同坐标
/// 且跨度重叠，同层间的并行边沿侧向错开，避免画在同一条线上。
/// 评分字典序 (节点冲突数, 是否占用主干, |偏移|)，节点避让优先于边间避让
/// （与 Spline 通道的两轮严格/放宽评估一致）。
fn orthogonal_route(
    s: &GGNode,
    t: &GGNode,
    s_slot: &(Port, Slot),
    t_slot: &(Port, Slot),
    obstacles: &Obstacles,
    used_trunks: &mut Vec<(f64, f64, f64)>,
) -> RoutePath {
    let (sp, tp) = (s_slot.0, t_slot.0);
    let start = port_point_at(s, sp, s_slot.1);
    let end = port_point_at(t, tp, t_slot.1);
    let skip = edge_skip(&s.id, &t.id, &obstacles.members);
    let vertical_main = matches!(sp, Port::Top | Port::Bottom);

    // 出/入 stub 端点（沿端口法向）。
    let (a, b) = if vertical_main {
        let out = if sp == Port::Bottom { STUB } else { -STUB };
        let inn = if tp == Port::Top { -STUB } else { STUB };
        (
            Point::new(start.x, start.y + out),
            Point::new(end.x, end.y + inn),
        )
    } else {
        let out = if sp == Port::Right { STUB } else { -STUB };
        let inn = if tp == Port::Left { -STUB } else { STUB };
        (
            Point::new(start.x + out, start.y),
            Point::new(end.x + inn, end.y),
        )
    };
    let base_trunk = if vertical_main {
        (a.x + b.x) / 2.0
    } else {
        (a.y + b.y) / 2.0
    };
    // 主干沿主轴方向的跨度（垂直主干 = y 跨度；水平主干 = x 跨度）。
    let (s0, s1) = if vertical_main {
        (a.y.min(b.y), a.y.max(b.y))
    } else {
        (a.x.min(b.x), a.x.max(b.x))
    };

    let build = |trunk: f64| -> Vec<Point> {
        if vertical_main {
            // 垂直主干：在 stub 端点高度接入主干，保持全程曼哈顿。
            vec![
                start,
                a,
                Point::new(trunk, a.y),
                Point::new(trunk, b.y),
                b,
                end,
            ]
        } else {
            vec![
                start,
                a,
                Point::new(a.x, trunk),
                Point::new(b.x, trunk),
                b,
                end,
            ]
        }
    };

    let mut candidates: Vec<f64> = vec![base_trunk];
    for step in 1..=MAX_OFFSET_TRIES {
        candidates.push(base_trunk + step as f64 * OFFSET_STEP);
        candidates.push(base_trunk - step as f64 * OFFSET_STEP);
    }
    let mut best: Option<(usize, bool, f64, Vec<Point>)> = None;
    for trunk in candidates {
        let pts = build(trunk);
        let c = polyline_conflicts(&pts, &skip, &obstacles.rects);
        let used = trunk_conflicts_used(trunk, s0, s1, used_trunks);
        let better = match &best {
            None => true,
            Some((bc, bu, bt, _)) => {
                c < *bc
                    || (c == *bc && (!used && *bu))
                    || (c == *bc
                        && used == *bu
                        && (trunk - base_trunk).abs() < (bt - base_trunk).abs())
            }
        };
        if better {
            best = Some((c, used, trunk, pts));
        }
        if c == 0 && !used {
            break;
        }
    }
    let Some((c, _, trunk, pts)) = best else {
        return points_to_route(&build(base_trunk));
    };
    // 主干不穿节点时才登记占用（穿节点的坏主干不占用，避免影响其它边）。
    if c == 0 {
        used_trunks.push((trunk, s0, s1));
    }
    points_to_route(&pts)
}

// ============================================================
// 端口
// ============================================================

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Port {
    Top,
    Bottom,
    Left,
    Right,
}

/// 端口法向（离开节点方向）单位向量。
fn port_normal(p: Port) -> (f64, f64) {
    match p {
        Port::Top => (0.0, -1.0),
        Port::Bottom => (0.0, 1.0),
        Port::Left => (-1.0, 0.0),
        Port::Right => (1.0, 0.0),
    }
}

/// 节点包围盒（含少量 padding 以避免边贴边）。
fn node_rect(n: &GGNode) -> Rect {
    let half_w = n.size.width / 2.0 + 2.0;
    let half_h = n.size.height / 2.0 + 2.0;
    Rect::new(
        n.center.x - half_w,
        n.center.y - half_h,
        n.center.x + half_w,
        n.center.y + half_h,
    )
}

/// 选端口：从 `from` 节点朝 `toward` 节点出线（或入线）时，用哪一面。
///
/// 规则：
/// - **优先沿布局主轴**（TB/BT → Top/Bottom；LR/RL → Left/Right），让 fan-out/fan-in
///   的多条边在同一条边上分散，符合流程图的流动方向；
/// - **当对端几乎位于侧方时切换到侧面端口**（`SIDE_BIAS` 判据：两节点中心连线与
///   节点矩形边界的交点落在哪条边）。否则 TB 布局里同层横向相邻的两点会被迫
///   "从底边出、绕到下方、再回到对面顶边"，走一大圈。
fn pick_port(from: &GGNode, toward: &GGNode, direction: crate::ast::Direction) -> Port {
    use crate::ast::Direction;
    const SIDE_BIAS: f64 = 2.5;
    let dx = toward.center.x - from.center.x;
    let dy = toward.center.y - from.center.y;
    let hw = (from.size.width / 2.0).max(1.0);
    let hh = (from.size.height / 2.0).max(1.0);

    match direction {
        // TB/BT：主轴垂直；侧向（水平）显著时改用 Left/Right。
        Direction::TB | Direction::TD | Direction::BT => {
            if dx.abs() * hh > dy.abs() * hw * SIDE_BIAS {
                if dx >= 0.0 { Port::Right } else { Port::Left }
            } else if dy >= 0.0 {
                Port::Bottom
            } else {
                Port::Top
            }
        }
        // LR/RL：主轴水平；侧向（垂直）显著时改用 Top/Bottom。
        Direction::LR | Direction::RL => {
            if dy.abs() * hw > dx.abs() * hh * SIDE_BIAS {
                if dy >= 0.0 { Port::Bottom } else { Port::Top }
            } else if dx >= 0.0 {
                Port::Right
            } else {
                Port::Left
            }
        }
    }
}

/// back edge 检测：source 在主轴方向上位于 target 「下游」（逆主轴）。
fn is_back_edge(s: &GGNode, t: &GGNode, direction: crate::ast::Direction) -> bool {
    use crate::ast::Direction;
    match direction {
        Direction::TB | Direction::TD => s.center.y > t.center.y,
        Direction::BT => s.center.y < t.center.y,
        Direction::LR => s.center.x > t.center.x,
        Direction::RL => s.center.x < t.center.x,
    }
}

/// 通道绕行边的端口：垂直主轴用侧端口（Right/Left），水平主轴用顶/底端口。
fn channel_ports(side: f64, direction: crate::ast::Direction) -> (Port, Port) {
    use crate::ast::Direction;
    let vert = matches!(direction, Direction::TB | Direction::TD | Direction::BT);
    if vert {
        if side > 0.0 {
            (Port::Right, Port::Right)
        } else {
            (Port::Left, Port::Left)
        }
    } else if side > 0.0 {
        (Port::Bottom, Port::Bottom)
    } else {
        (Port::Top, Port::Top)
    }
}

/// 一条边实际使用的（出端口, 入端口）。
///
/// back edge 走专用端口（沿主轴反向绕行），普通边由 [`pick_port`] 双向决定。
/// 统计槽位与生成路由**必须**共用此函数，否则槽位序号会与端口错配。
fn ports_for(
    s: &GGNode,
    t: &GGNode,
    direction: crate::ast::Direction,
    is_back: bool,
) -> (Port, Port) {
    use crate::ast::Direction;
    if is_back {
        match direction {
            Direction::TB | Direction::TD => (Port::Top, Port::Bottom),
            Direction::BT => (Port::Bottom, Port::Top),
            Direction::LR => (Port::Right, Port::Left),
            Direction::RL => (Port::Left, Port::Right),
        }
    } else {
        (pick_port(s, t, direction), pick_port(t, s, direction))
    }
}

/// 对端节点在 `port` 所在边**轴向**上的投影坐标，用于给同一端口上的多条边排序。
/// Top/Bottom 边沿 x 展开 → 取 x；Left/Right 边沿 y 展开 → 取 y。
fn axis_projection(port: Port, peer: Point) -> f64 {
    match port {
        Port::Top | Port::Bottom => peer.x,
        Port::Left | Port::Right => peer.y,
    }
}

/// 组内按对端投影排序 → 写入槽位 (idx, total)。
fn rank_slots(
    groups: HashMap<(String, Port), Vec<(usize, f64)>>,
    slots: &mut [(Slot, Slot)],
    is_src: bool,
) {
    use std::cmp::Ordering;
    for (_, mut members) in groups {
        members.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(Ordering::Equal)
                .then(a.0.cmp(&b.0))
        });
        let total = members.len();
        for (idx, (edge_i, _)) in members.into_iter().enumerate() {
            let slot = Slot { idx, total };
            if is_src {
                slots[edge_i].0 = slot;
            } else {
                slots[edge_i].1 = slot;
            }
        }
    }
}

/// 节点边上按槽位分散的端口点（形状感知：端口永远落在形状边界上）。
fn port_point_at(n: &GGNode, p: Port, slot: Slot) -> Point {
    let span = match p {
        Port::Top | Port::Bottom => n.size.width,
        Port::Left | Port::Right => n.size.height,
    };
    if slot.total <= 1 {
        return port_point_offset(n, p, 0.0);
    }
    // 分散间距：先把边长按边数均分（铺满整条边），再 clamp：
    // - 下限 PORT_MIN_SPACING：节点很窄 / 边很多时不至于重叠成一团；
    // - 上限 span * PORT_MAX_SPACING_RATIO：宽节点上的少数几条边也能真正分散开。
    // 上限需不小于下限：节点很窄（span < PORT_MIN_SPACING / RATIO）时，
    // `span * RATIO` 会小于下限，导致 clamp 上下限倒置 panic。
    let mut spacing = (span / slot.total as f64).clamp(
        PORT_MIN_SPACING,
        (span * PORT_MAX_SPACING_RATIO).max(PORT_MIN_SPACING),
    );
    // 总跨度不得超出「边长 - 两侧留白」，避免端口点贴到节点角上。
    let usable = (span - 2.0 * PORT_CORNER_MARGIN).max(PORT_MIN_SPACING);
    spacing = spacing.min(usable / (slot.total - 1) as f64);
    // 居中对称：idx 从 -(total-1)/2 .. +(total-1)/2
    let offset = (slot.idx as f64 - (slot.total as f64 - 1.0) / 2.0) * spacing;
    port_point_offset(n, p, offset)
}

/// 端口点：节点 `p` 面上、沿该面偏移 `offset` 后的边界点。
///
/// 形状感知：
/// - Rectangle（及默认）：端口在包围盒边上；
/// - Diamond：端口投影到菱形边界 `|dx|/hw + |dy|/hh = 1`（否则端口悬在包围盒
///   与菱形之间的空隙里，连线"连不上"菱形）；
/// - Circle / DoubleCircle / StartDot / EndDot：投影到内切椭圆边界。
fn port_point_offset(n: &GGNode, p: Port, offset: f64) -> Point {
    let (hw, hh) = (n.size.width / 2.0, n.size.height / 2.0);
    let (cx, cy) = (n.center.x, n.center.y);
    match n.shape {
        ShapeKind::Diamond => match p {
            Port::Top | Port::Bottom => {
                let dx = offset.clamp(-hw * 0.75, hw * 0.75);
                let sign = if p == Port::Top { -1.0 } else { 1.0 };
                let y = hh * (1.0 - dx.abs() / hw.max(1e-9));
                Point::new(cx + dx, cy + sign * y)
            }
            Port::Left | Port::Right => {
                let dy = offset.clamp(-hh * 0.75, hh * 0.75);
                let sign = if p == Port::Left { -1.0 } else { 1.0 };
                let x = hw * (1.0 - dy.abs() / hh.max(1e-9));
                Point::new(cx + sign * x, cy + dy)
            }
        },
        ShapeKind::Circle | ShapeKind::DoubleCircle | ShapeKind::StartDot | ShapeKind::EndDot => {
            match p {
                Port::Top | Port::Bottom => {
                    let dx = offset.clamp(-hw * 0.9, hw * 0.9);
                    let sign = if p == Port::Top { -1.0 } else { 1.0 };
                    let t = (1.0 - (dx / hw.max(1e-9)).powi(2)).max(0.0).sqrt();
                    Point::new(cx + dx, cy + sign * hh * t)
                }
                Port::Left | Port::Right => {
                    let dy = offset.clamp(-hh * 0.9, hh * 0.9);
                    let sign = if p == Port::Left { -1.0 } else { 1.0 };
                    let t = (1.0 - (dy / hh.max(1e-9)).powi(2)).max(0.0).sqrt();
                    Point::new(cx + sign * hw * t, cy + dy)
                }
            }
        }
        _ => match p {
            Port::Top => Point::new(cx + offset, cy - hh),
            Port::Bottom => Point::new(cx + offset, cy + hh),
            Port::Left => Point::new(cx - hw, cy + offset),
            Port::Right => Point::new(cx + hw, cy + offset),
        },
    }
}

// ============================================================
// 几何工具
// ============================================================

/// 矩形转置 (x, y) 互换。
fn tr_rect(r: Rect) -> Rect {
    Rect::new(r.min_y(), r.min_x(), r.max_y(), r.max_x())
}

/// 两矩形是否相交（AABB）。
fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.min_x() <= b.max_x()
        && a.max_x() >= b.min_x()
        && a.min_y() <= b.max_y()
        && a.max_y() >= b.min_y()
}

/// 折线是否完全避开非端点节点 / 非成员容器（true = 无冲突）。
fn polyline_clear(
    pts: &[Point],
    skip: &dyn Fn(&str) -> bool,
    rects: &[(usize, Rect, String)],
) -> bool {
    for w in pts.windows(2) {
        for (_, r, owner) in rects {
            if skip(owner) {
                continue;
            }
            if segment_intersects_rect(w[0], w[1], r) {
                return false;
            }
        }
    }
    true
}

/// 折线穿过非端点节点 / 非成员容器的段数。
fn polyline_conflicts(
    pts: &[Point],
    skip: &dyn Fn(&str) -> bool,
    rects: &[(usize, Rect, String)],
) -> usize {
    pts.windows(2)
        .filter(|w| {
            rects
                .iter()
                .any(|(_, r, owner)| !skip(owner) && segment_intersects_rect(w[0], w[1], r))
        })
        .count()
}

/// 把折线点序列转成路由段序列（跳过重复点；相邻段合并共线）。
fn points_to_route(points: &[Point]) -> RoutePath {
    let mut pts: Vec<Point> = Vec::new();
    for &p in points {
        if let Some(last) = pts.last()
            && (p.x - last.x).abs() < 1e-9
            && (p.y - last.y).abs() < 1e-9
        {
            continue;
        }
        // 与倒数第二点共线则替换（用更远点，保持方向）。
        if pts.len() >= 2 {
            let a = pts[pts.len() - 2];
            let b = *pts.last().unwrap();
            if ((b.x - a.x) * (p.y - b.y) - (b.y - a.y) * (p.x - b.x)).abs() < 1e-6 {
                *pts.last_mut().unwrap() = p;
                continue;
            }
        }
        pts.push(p);
    }
    crate::builder::ir::geograph::line_route(&pts)
}

/// 圆角折线路由：每个拐点用二次贝塞尔（升阶为三次）平滑过渡，
/// 其余部分保持直线。用于通道绕行路径，观感接近官方的平滑长边。
fn rounded_polyline(pts: &[Point], radius: f64) -> RoutePath {
    let mut r = RoutePath::new();
    if pts.len() < 2 {
        return crate::builder::ir::geograph::line_route(pts);
    }
    let mut cur = pts[0];
    for i in 1..pts.len().saturating_sub(1) {
        let corner = pts[i];
        let next = pts[i + 1];
        let (d1x, d1y) = (corner.x - cur.x, corner.y - cur.y);
        let (d2x, d2y) = (next.x - corner.x, next.y - corner.y);
        let l1 = (d1x * d1x + d1y * d1y).sqrt();
        let l2 = (d2x * d2x + d2y * d2y).sqrt();
        if l1 < 1e-9 || l2 < 1e-9 {
            continue;
        }
        let rr = radius.min(l1 * 0.5).min(l2 * 0.5);
        let cs = Point::new(corner.x - d1x / l1 * rr, corner.y - d1y / l1 * rr);
        let ce = Point::new(corner.x + d2x / l2 * rr, corner.y + d2y / l2 * rr);
        if (cs.x - cur.x).abs() > 1e-6 || (cs.y - cur.y).abs() > 1e-6 {
            r.push(RouteSegment::Line { from: cur, to: cs });
        }
        // 二次贝塞尔 (cs, corner, ce) 升阶为三次。
        let p1 = Point::new(
            cs.x + (corner.x - cs.x) * 2.0 / 3.0,
            cs.y + (corner.y - cs.y) * 2.0 / 3.0,
        );
        let p2 = Point::new(
            ce.x + (corner.x - ce.x) * 2.0 / 3.0,
            ce.y + (corner.y - ce.y) * 2.0 / 3.0,
        );
        r.push(RouteSegment::CubicBezier {
            p0: cs,
            p1,
            p2,
            p3: ce,
        });
        cur = ce;
    }
    let last = *pts.last().unwrap();
    if (last.x - cur.x).abs() > 1e-6 || (last.y - cur.y).abs() > 1e-6 {
        r.push(RouteSegment::Line {
            from: cur,
            to: last,
        });
    }
    r
}

/// 把路由采样成折线点序列（贝塞尔按 `n` 等分采样），用于碰撞检测。
fn sample_path(r: &RoutePath, n: usize) -> Vec<Point> {
    let mut out = Vec::new();
    for seg in r.iter() {
        match *seg {
            RouteSegment::Line { from, to } => {
                for i in 0..=n {
                    let t = i as f64 / n as f64;
                    out.push(Point::new(
                        from.x + (to.x - from.x) * t,
                        from.y + (to.y - from.y) * t,
                    ));
                }
            }
            RouteSegment::CubicBezier { p0, p1, p2, p3 } => {
                for i in 0..=n {
                    let t = i as f64 / n as f64;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    out.push(Point::new(
                        a * p0.x + b * p1.x + c * p2.x + d * p3.x,
                        a * p0.y + b * p1.y + c * p2.y + d * p3.y,
                    ));
                }
            }
        }
    }
    out
}

use super::spatial::segment_intersects_rect;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::ir::common::{
        ArrowKind, ArrowSpec, ContainerKind, NodeDetail, NodeRole, ResolvedPorts,
    };
    use crate::builder::ir::geograph::{GGContainer, GGEdge, GGNode, Geograph};
    use crate::builder::ir::shape::ShapeKind;
    use crate::builder::ir::unigraph::EdgeKind;

    fn mk_node(id: &str, x: f64, y: f64, w: f64, h: f64) -> GGNode {
        let p = Point::new(x, y);
        GGNode {
            id: id.to_string(),
            role: NodeRole::Atom,
            center: p,
            size: lievisual::geometry::Size::new(w, h),
            shape: ShapeKind::Rectangle,
            ports: ResolvedPorts {
                top: Point::new(x, y - h / 2.0),
                bottom: Point::new(x, y + h / 2.0),
                left: Point::new(x - w / 2.0, y),
                right: Point::new(x + w / 2.0, y),
            },
            label: None,
            detail: NodeDetail::None,
        }
    }

    fn mk_edge(s: &str, t: &str) -> GGEdge {
        GGEdge {
            id: format!("{}-{}", s, t),
            source: s.to_string(),
            target: t.to_string(),
            route: RoutePath::new(),
            label_text: None,
            label_anchor: None,
            kind: EdgeKind::Flow,
            arrow: ArrowSpec {
                start: ArrowKind::None,
                end: ArrowKind::Arrow,
            },
            routing_hint: crate::builder::ir::common::RoutingHint::Orthogonal,
            line_kind: crate::builder::ir::common::LineKind::Solid,
            cardinality: (None, None),
            cardinality_text: (None, None),
        }
    }

    /// 线段相交判定（含端点接触）。
    fn segs_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
        let cross =
            |o: Point, p: Point, q: Point| (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
        let d1 = cross(c, d, a);
        let d2 = cross(c, d, b);
        let d3 = cross(a, b, c);
        let d4 = cross(a, b, d);
        ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
    }

    /// fan-out：一个源节点扇出到同层横向铺开的多个目标（层间距小、横向跨度大）。
    ///
    /// 回归用例：旧实现按 `max(|dx|,|dy|)` 取控制点弧度，会把控制点甩到目标行之外
    /// 形成 S 形回绕，导致相邻出边互相穿过。要求：
    /// 1. 曲线整体落在两行之间的走廊内（不回绕到源/目标行之外）；
    /// 2. 任意两条出边互不相交。
    #[test]
    fn fan_out_spline_edges_stay_in_corridor_and_never_cross() {
        use crate::ast::Direction;
        let xs = [-336.0, -168.0, 0.0, 168.0, 336.0];
        let mut nodes = vec![mk_node("A", 0.0, 0.0, 120.0, 60.0)];
        for (i, x) in xs.iter().enumerate() {
            nodes.push(mk_node(&format!("C{i}"), *x, 116.0, 120.0, 60.0));
        }
        let mut edges = Vec::new();
        for (i, _) in xs.iter().enumerate() {
            let mut e = mk_edge("A", &format!("C{i}"));
            e.routing_hint = crate::builder::ir::common::RoutingHint::Spline;
            edges.push(e);
        }
        let mut gg = Geograph {
            size: lievisual::geometry::Size::new(0.0, 0.0),
            background: lievisual::geometry::Color::default(),
            nodes,
            edges,
            containers: vec![],
            title: None,
            show_data: false,
            activations: vec![],
            sequence_dividers: vec![],
        };
        route_edges(&mut gg, Direction::TB, &[]);

        // A 底边 y = 30，子节点顶边 y = 86 → 走廊 [30, 86]
        for e in &gg.edges {
            for p in sample_path(&e.route, 48) {
                assert!(
                    p.y >= 30.0 - 1e-6 && p.y <= 86.0 + 1e-6,
                    "出边 {}-{} 逸出层间走廊: y={} (期望 30..=86)",
                    e.source,
                    e.target,
                    p.y
                );
            }
        }

        // 两两不相交
        let samples: Vec<Vec<Point>> = gg.edges.iter().map(|e| sample_path(&e.route, 48)).collect();
        for i in 0..samples.len() {
            for j in (i + 1)..samples.len() {
                for k in 0..samples[i].len() - 1 {
                    for l in 0..samples[j].len() - 1 {
                        assert!(
                            !segs_cross(
                                samples[i][k],
                                samples[i][k + 1],
                                samples[j][l],
                                samples[j][l + 1]
                            ),
                            "出边 {} 与 {} 发生交叉: seg1 {:?}->{:?}, seg2 {:?}->{:?}",
                            gg.edges[i].target,
                            gg.edges[j].target,
                            samples[i][k],
                            samples[i][k + 1],
                            samples[j][l],
                            samples[j][l + 1]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn orthogonal_route_has_orthogonal_segments_and_avoids_node() {
        // A 在左、B 在右，中间 C 阻挡
        // A(0,0) - B(200,0)，C(100,0) 正好在连线上
        let mut gg = Geograph {
            size: lievisual::geometry::Size::new(0.0, 0.0),
            background: lievisual::geometry::Color::default(),
            nodes: vec![
                mk_node("A", 0.0, 0.0, 60.0, 40.0),
                mk_node("B", 200.0, 0.0, 60.0, 40.0),
                mk_node("C", 100.0, 0.0, 60.0, 40.0),
            ],
            edges: vec![mk_edge("A", "B")],
            containers: vec![],
            title: None,
            show_data: false,
            activations: vec![],
            sequence_dividers: vec![],
        };
        route_edges(&mut gg, crate::ast::Direction::TB, &[]);
        let route = &gg.edges[0].route;
        // 正交折线应 >= 3 个段（含 stub + 中间拐点）
        assert!(route.len() >= 3, "正交路由段过少: {:?}", route);

        // 路由不应穿过 C 的包围盒（除非 C 是端点，但 C 不是）
        let c_rect = Rect::new(100.0 - 32.0, 0.0 - 22.0, 100.0 + 32.0, 0.0 + 22.0);
        for seg in route.iter() {
            let (a, b) = (seg.start(), seg.end());
            assert!(
                !segment_intersects_rect(a, b, &c_rect),
                "路由穿越中间阻挡节点 C: {:?} -> {:?}",
                a,
                b
            );
        }
    }

    #[test]
    fn route_preserves_endpoints() {
        let mut gg = Geograph {
            size: lievisual::geometry::Size::new(0.0, 0.0),
            background: lievisual::geometry::Color::default(),
            nodes: vec![
                mk_node("A", 0.0, 0.0, 60.0, 40.0),
                mk_node("B", 0.0, 150.0, 60.0, 40.0),
            ],
            edges: vec![mk_edge("A", "B")],
            containers: vec![],
            title: None,
            show_data: false,
            activations: vec![],
            sequence_dividers: vec![],
        };
        route_edges(&mut gg, crate::ast::Direction::TB, &[]);
        let route = &gg.edges[0].route;
        // 垂直主导：A 出 Bottom 端口(0,20)，B 入 Top 端口(0,130)
        let start = route.start();
        assert!((start.x - 0.0).abs() < 1.0, "起点 x 应≈0");
        assert!(
            start.y > 0.0 && start.y < 40.0,
            "起点应在 A 底部出线, got {:?}",
            start
        );
        let end = route.end();
        assert!(
            end.y > 110.0 && end.y < 150.0,
            "终点应在 B 顶部入线, got {:?}",
            end
        );
    }

    /// 自环路由：应生成外凸环（非退化路径），起止点都在节点右边缘。
    #[test]
    fn self_loop_route_bulges_right() {
        let mut gg = Geograph {
            size: lievisual::geometry::Size::new(0.0, 0.0),
            background: lievisual::geometry::Color::default(),
            nodes: vec![mk_node("B", 0.0, 0.0, 120.0, 60.0)],
            edges: vec![mk_edge("B", "B")],
            containers: vec![],
            title: None,
            show_data: false,
            activations: vec![],
            sequence_dividers: vec![],
        };
        route_edges(&mut gg, crate::ast::Direction::TB, &[]);
        let route = &gg.edges[0].route;
        assert!(!route.is_empty(), "自环路由不应为空");
        let samples = sample_path(route, 32);
        // 环应伸到节点右边缘之外
        let max_x = samples
            .iter()
            .map(|p| p.x)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(max_x > 60.0 + 10.0, "自环应外凸出右边缘, max_x={max_x}");
        // 起止点应在节点右边缘上
        assert!((route.start().x - 60.0).abs() < 1e-6);
        assert!((route.end().x - 60.0).abs() < 1e-6);
    }

    /// 双向对：两条相对边应一左一右分开（不重合）。
    #[test]
    fn mutual_pair_edges_are_separated() {
        use crate::ast::Direction;
        let mut gg = Geograph {
            size: lievisual::geometry::Size::new(0.0, 0.0),
            background: lievisual::geometry::Color::default(),
            nodes: vec![
                mk_node("B", 0.0, 0.0, 120.0, 60.0),
                mk_node("C", 0.0, 140.0, 120.0, 60.0),
            ],
            edges: vec![mk_edge("B", "C"), mk_edge("C", "B")],
            containers: vec![],
            title: None,
            show_data: false,
            activations: vec![],
            sequence_dividers: vec![],
        };
        gg.edges[0].routing_hint = crate::builder::ir::common::RoutingHint::Spline;
        gg.edges[1].routing_hint = crate::builder::ir::common::RoutingHint::Spline;
        route_edges(&mut gg, Direction::TB, &[]);
        let x0 = gg.edges[0].route.start().x;
        let x1 = gg.edges[1].route.start().x;
        assert!(
            (x0 - x1).abs() > 2.0,
            "双向对两条边应左右分开: {x0} vs {x1}"
        );
        assert!(
            (x0 - 0.0).abs() < 20.0 && (x1 - 0.0).abs() < 20.0,
            "双向对偏移应很小（紧贴风格）"
        );
    }

    /// 菱形端口：端口点应落在菱形边界上（而非包围盒角部悬空）。
    #[test]
    fn diamond_ports_lie_on_shape_boundary() {
        let n = mk_node("D", 0.0, 0.0, 200.0, 120.0);
        let mut n = n;
        n.shape = ShapeKind::Diamond;
        // 底面偏移 66（约 hw/3）：边界 y = hh * (1 - 66/100) = 19.8
        let p = port_point_offset(&n, Port::Bottom, 66.0);
        let ratio = (p.x / 100.0).abs() + (p.y / 60.0).abs();
        assert!(
            (ratio - 1.0).abs() < 1e-6,
            "端口应落在菱形边界上, ratio={ratio}"
        );
        // 顶面中点 = 顶点
        let top = port_point_offset(&n, Port::Top, 0.0);
        assert!((top.y - (-60.0)).abs() < 1e-6 && (top.x - 0.0).abs() < 1e-6);
    }

    /// 0.3 容器避障：两端点均非容器成员时，普通边应绕开容器框（不得穿过）。
    #[test]
    fn normal_edge_avoids_container() {
        use crate::ast::Direction;
        // 容器框覆盖 A 与 B 之间走廊的中部（x∈[-40,40], y∈[-20,100]），
        // 两侧都留有足够的空白走廊（A 上方 / B 下方），侧边通道可绕行。
        let container = GGContainer {
            id: "sub1".into(),
            bounds: Rect::new(-40.0, -20.0, 40.0, 100.0),
            title: None,
            kind: ContainerKind::Subgraph,
            member_ids: vec![],
        };
        let mut gg = Geograph {
            size: lievisual::geometry::Size::new(0.0, 0.0),
            background: lievisual::geometry::Color::default(),
            nodes: vec![
                mk_node("A", 0.0, -140.0, 60.0, 40.0),
                mk_node("B", 0.0, 200.0, 60.0, 40.0),
            ],
            edges: vec![mk_edge("A", "B")],
            containers: vec![],
            title: None,
            show_data: false,
            activations: vec![],
            sequence_dividers: vec![],
        };
        gg.edges[0].routing_hint = crate::builder::ir::common::RoutingHint::Spline;
        route_edges(&mut gg, Direction::TB, &[container]);
        let route = &gg.edges[0].route;
        // 朴素 Spline 会沿 x≈0 直线穿过容器；避障后任何采样点不得落入容器框内部。
        for p in sample_path(route, 64) {
            assert!(
                !(p.x > -40.0 && p.x < 40.0 && p.y > -20.0 && p.y < 100.0),
                "边 A->B 穿过容器框: {p:?}"
            );
        }
    }

    /// 0.3 容器避障：容器内边（两端点均为成员）允许保持原路由，不绕行。
    #[test]
    fn intra_container_edge_stays_inside() {
        use crate::ast::Direction;
        let container = GGContainer {
            id: "sub1".into(),
            bounds: Rect::new(-80.0, -30.0, 80.0, 130.0),
            title: None,
            kind: ContainerKind::Subgraph,
            member_ids: vec!["A".into(), "B".into()],
        };
        let mut gg = Geograph {
            size: lievisual::geometry::Size::new(0.0, 0.0),
            background: lievisual::geometry::Color::default(),
            nodes: vec![
                mk_node("A", 0.0, 0.0, 60.0, 40.0),
                mk_node("B", 0.0, 80.0, 60.0, 40.0),
            ],
            edges: vec![mk_edge("A", "B")],
            containers: vec![],
            title: None,
            show_data: false,
            activations: vec![],
            sequence_dividers: vec![],
        };
        gg.edges[0].routing_hint = crate::builder::ir::common::RoutingHint::Spline;
        route_edges(&mut gg, Direction::TB, &[container]);
        // 朴素 Spline 不穿过任何障碍 → 应保持朴素路由（单段贝塞尔），不生成绕行折线。
        assert_eq!(gg.edges[0].route.len(), 1, "容器内边不应被绕行");
    }

    /// 0.6 正交边-边避让：同层间的并行正交边主干不得重叠（同 x 且 y 跨度交叠）。
    #[test]
    fn orthogonal_edges_do_not_share_trunk() {
        use crate::ast::Direction;
        // 交叉对：A1→B1 与 A2→B2 的主干中线都为 x=0 且跨度重叠，
        // 若无边间避让，两条边会画在同一条垂直主干上。
        let mut gg = Geograph {
            size: lievisual::geometry::Size::new(0.0, 0.0),
            background: lievisual::geometry::Color::default(),
            nodes: vec![
                mk_node("A1", -30.0, 0.0, 60.0, 40.0),
                mk_node("A2", 30.0, 0.0, 60.0, 40.0),
                mk_node("B1", 30.0, 200.0, 60.0, 40.0),
                mk_node("B2", -30.0, 200.0, 60.0, 40.0),
            ],
            edges: vec![mk_edge("A1", "B1"), mk_edge("A2", "B2")],
            containers: vec![],
            title: None,
            show_data: false,
            activations: vec![],
            sequence_dividers: vec![],
        };
        route_edges(&mut gg, Direction::TB, &[]);
        // 收集所有长垂直主干段（stub 长 18 < 40，只捕获主干）。
        let mut trunk_x: Vec<f64> = Vec::new();
        for e in &gg.edges {
            for seg in e.route.iter() {
                if let RouteSegment::Line { from, to } = seg
                    && (from.x - to.x).abs() < 1e-6
                    && (from.y - to.y).abs() > 40.0
                {
                    trunk_x.push(from.x);
                }
            }
        }
        assert!(trunk_x.len() >= 2, "应有至少两条正交主干: {trunk_x:?}");
        for i in 0..trunk_x.len() {
            for j in (i + 1)..trunk_x.len() {
                assert!(
                    (trunk_x[i] - trunk_x[j]).abs() > 2.0,
                    "两条正交主干重叠: {trunk_x:?}"
                );
            }
        }
    }
}
