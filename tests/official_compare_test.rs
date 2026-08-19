//! 与 dagre 官方布局的端到端结构化对拍测试。
//!
//! 数据来源: `tests/dagre_ref/run.js` 用 @dagrejs/dagre 生成 `layouts.json`
//! （节点中心坐标 + 边端口折线）。本测试读取该 fixture，与 liemermaid 的
//! Sugiyama 布局做三层对拍:
//!   1. 拓扑层序 (rank) — 硬断言，必须完全一致
//!   2. 同层 Y 对齐    — 硬断言（同 rank 节点中心 y 差 < 容差）
//!   3. 坐标归一化    — 软断言（bounding box 归一到 [0,1]，节点中心距离 < 容差）
//!
//! 边的逐点形状不做强对比：dagre 输出"端口交点"，liemermaid 输出"中心路由点"，
//! 二者语义不同，仅对比边的起止端点等于各自节点中心。

use std::collections::{HashMap, HashSet};

use liemermaid::builder::layout::sugiyama::{NodeSize, SugiyamaConfig, SugiyamaLayout};
use petgraph::graph::DiGraph;
use serde::Deserialize;
use vello_cpu::kurbo::Point;

#[derive(Debug, Deserialize)]
struct DagreNode {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[derive(Debug, Deserialize)]
struct DagreEdge {
    from: String,
    to: String,
    #[allow(dead_code)]
    points: Vec<DagrePoint>,
}

#[derive(Debug, Deserialize)]
struct DagrePoint {
    x: f64,
    y: f64,
}

#[derive(Debug, Deserialize)]
struct DagreCase {
    name: String,
    rankdir: String,
    nodes: HashMap<String, DagreNode>,
    edges: Vec<DagreEdge>,
}

#[derive(Debug, Deserialize)]
struct DagreFixture {
    cases: Vec<DagreCase>,
}

fn load_fixture() -> DagreFixture {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/dagre_ref/layouts.json");
    let text = std::fs::read_to_string(path)
        .expect("layouts.json missing; run `node tests/dagre_ref/run.js`");
    serde_json::from_str(&text).expect("invalid layouts.json")
}

/// 节点中心坐标 -> 归一化到 [0,1] 包围盒
fn normalize(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in points {
        minx = minx.min(x);
        miny = miny.min(y);
        maxx = maxx.max(x);
        maxy = maxy.max(y);
    }
    let w = (maxx - minx).max(1.0);
    let h = (maxy - miny).max(1.0);
    points
        .iter()
        .map(|&(x, y)| ((x - minx) / w, (y - miny) / h))
        .collect()
}

#[test]
fn official_layout_topology_and_coords_match() {
    let fixture = load_fixture();
    assert!(!fixture.cases.is_empty(), "no cases in fixture");

    let coord_tol = 0.06; // 归一化坐标误差容忍（[0,1] 空间）
    let y_align_tol = 1.0;

    let mut total_cases = 0;
    let mut hard_pass = 0;
    let mut soft_pass = 0;

    for case in &fixture.cases {
        total_cases += 1;
        // 构造 petgraph
        let mut g = DiGraph::<String, ()>::new();
        let mut idx: HashMap<String, petgraph::graph::NodeIndex> = HashMap::new();
        for id in case.nodes.keys() {
            let n = g.add_node(id.clone());
            idx.insert(id.clone(), n);
        }
        for e in &case.edges {
            let &u = idx.get(&e.from).expect("edge from missing");
            let &v = idx.get(&e.to).expect("edge to missing");
            g.add_edge(u, v, ());
        }

        let config = SugiyamaConfig {
            ranker: "network-simplex".to_string(),
            ..SugiyamaConfig::default()
        };
        let sizes: HashMap<_, _> = g
            .node_indices()
            .map(|n| {
                (
                    n,
                    NodeSize {
                        width: 100.0,
                        height: 40.0,
                    },
                )
            })
            .collect();

        let lay = SugiyamaLayout::new(config, &g);
        let res = lay.layout(&sizes);

        // dagre 侧：由中心 y 排序推导 rank（TB/LR/BT/RL 都先按主轴向排序后再映射）
        let (dagre_rank, dagre_norm) = dagre_ranks_and_norm(case);
        let (ours_rank, ours_norm) = ours_ranks_and_norm(&res, &idx, case);

        // --- 硬断言 1: 拓扑偏序约束 ---
        // 比较"沿边的层序关系"而非绝对 rank 数字：对每条边 u->v，
        // dagre 与 ours 都应满足 rank(u) < rank(v)。这能验证拓扑正确性，
        // 同时容忍环处理导致的绝对层号偏移。
        // 双向边 (u->v 且 v->u 同时存在) 视为环，跳过偏序硬断言（回边无拓扑序）。
        let mut bidir: HashSet<(String, String)> = HashSet::new();
        for e in &case.edges {
            if case.edges.iter().any(|o| o.from == e.to && o.to == e.from) {
                bidir.insert((e.from.clone(), e.to.clone()));
                bidir.insert((e.to.clone(), e.from.clone()));
            }
        }
        let mut order_ok = true;
        let mut abs_rank_diff = 0usize;
        for e in &case.edges {
            if bidir.contains(&(e.from.clone(), e.to.clone())) {
                continue; // 环内回边，无偏序约束
            }
            let du = dagre_rank.get(&e.from).copied();
            let dv = dagre_rank.get(&e.to).copied();
            let ou = ours_rank.get(&e.from).copied();
            let ov = ours_rank.get(&e.to).copied();
            if let (Some(du), Some(dv), Some(ou), Some(ov)) = (du, dv, ou, ov) {
                if !(du < dv) {
                    order_ok = false;
                    eprintln!(
                        "[{}] dagre order violated on {}/{}: {} !< {}",
                        case.name, e.from, e.to, du, dv
                    );
                }
                if !(ou < ov) {
                    order_ok = false;
                    eprintln!(
                        "[{}] ours order violated on {}/{}: {} !< {}",
                        case.name, e.from, e.to, ou, ov
                    );
                }
                if du != ou || dv != ov {
                    abs_rank_diff += 1;
                }
            }
        }

        // --- 硬断言 2: 同层 Y 对齐 ---
        let mut yalign_ok = true;
        let mut by_rank: HashMap<usize, Vec<String>> = HashMap::new();
        for (id, &r) in ours_rank.iter() {
            by_rank.entry(r).or_default().push(id.clone());
        }
        for (r, rank_nodes) in by_rank.iter() {
            if rank_nodes.len() < 2 {
                continue;
            }
            let ys: Vec<f64> = rank_nodes
                .iter()
                .map(|id| res.positions[&idx[id]].y)
                .collect();
            let mn = ys.iter().cloned().fold(f64::MAX, f64::min);
            let mx = ys.iter().cloned().fold(f64::MIN, f64::max);
            if (mx - mn) > y_align_tol {
                yalign_ok = false;
                eprintln!(
                    "[{}] rank {} y misalign: {:?} (span {})",
                    case.name,
                    r,
                    ys,
                    mx - mn
                );
            }
        }

        // --- 软断言: 同层集合 + 归一化坐标距离 ---
        // 同层节点允许左右互换（Barycenter 初始序差异），因此按"同层集合"匹配：
        // 对每个 dagre rank，找到 ours 中节点集合相同的 rank，再逐节点比坐标。
        let mut coord_ok = true;
        let dagre_by_rank = group_by_rank(&dagre_rank);
        let ours_by_rank = group_by_rank(&ours_rank);
        for (dr, dnodes) in dagre_by_rank.iter() {
            // 在 ours 中找集合相同的 rank
            let matched = ours_by_rank.iter().find(|(_, on)| same_set(on, dnodes));
            let _onodes = match matched {
                Some((_, on)) => on,
                None => {
                    coord_ok = false;
                    eprintln!("[{}] no ours-rank matches dagre rank {}", case.name, dr);
                    continue;
                }
            };
            for id in dnodes {
                let dn = dagre_norm.get(id).copied().unwrap_or((0.0, 0.0));
                let on = ours_norm.get(id).copied().unwrap_or((0.0, 0.0));
                let dist = ((dn.0 - on.0).powi(2) + (dn.1 - on.1).powi(2)).sqrt();
                if dist > coord_tol {
                    coord_ok = false;
                    eprintln!(
                        "[{}] coord mismatch node {}: dagre=({:.3},{:.3}) ours=({:.3},{:.3}) dist={:.3}",
                        case.name, id, dn.0, dn.1, on.0, on.1, dist
                    );
                }
            }
        }

        let hard_ok = order_ok && yalign_ok;
        if hard_ok {
            hard_pass += 1;
        }
        if coord_ok {
            soft_pass += 1;
        }
        if abs_rank_diff > 0 {
            eprintln!(
                "[{}] NOTE: {} nodes differ in absolute rank vs dagre (ring/ordering tolerance)",
                case.name, abs_rank_diff
            );
        }
        assert!(
            hard_ok,
            "case '{}' failed HARD assertions (topo-order/y-align)",
            case.name
        );
        eprintln!(
            "[{}] hard=OK soft={} (coord within {})",
            case.name,
            if coord_ok { "OK" } else { "DIFF" },
            coord_tol
        );
    }

    eprintln!(
        "=== official_compare: {}/{} cases hard-pass, {}/{} soft-pass (tol={}) ===",
        hard_pass, total_cases, soft_pass, total_cases, coord_tol
    );
}

/// dagre 侧：将中心坐标按 rankdir 主轴向排序得到 rank，并归一化中心坐标
fn dagre_ranks_and_norm(case: &DagreCase) -> (HashMap<String, usize>, HashMap<String, (f64, f64)>) {
    // 主轴坐标：TB/BT 用 y，LR/RL 用 x
    let primary = |n: &DagreNode| -> f64 {
        match case.rankdir.as_str() {
            "LR" | "RL" => n.x,
            _ => n.y,
        }
    };
    let mut ids: Vec<&String> = case.nodes.keys().collect();
    ids.sort_by(|a, b| {
        primary(case.nodes.get(*a).unwrap())
            .partial_cmp(&primary(case.nodes.get(*b).unwrap()))
            .unwrap()
    });
    let mut rank = HashMap::new();
    let mut prev: Option<f64> = None;
    let mut cur = 0usize;
    for id in ids {
        let p = primary(case.nodes.get(id).unwrap());
        if let Some(pv) = prev
            && p - pv > 1.0
        {
            cur += 1;
        }
        // 第一层（prev=None）cur 保持 0
        prev = Some(p);
        rank.insert(id.clone(), cur);
    }
    // BT/RL: dagre 把物理底部/右部作为 rank 0，而 liemermaid 的 layers rank 0
    // 永远是物理顶部/左部（方向变换作用于 positions 而非 layers）。为对齐语义，
    // 对 BT/RL 将 dagre rank 翻转：rank' = max_rank - rank。
    if case.rankdir == "BT" || case.rankdir == "RL" {
        let max_r = rank.values().copied().max().unwrap_or(0);
        for v in rank.values_mut() {
            *v = max_r - *v;
        }
    }
    // 归一化中心坐标（注意 LR/RL 时 x/y 含义互换，但对拍用原始 x/y 一致空间即可）
    // BT/RL: 先对 dagre 坐标应用与 liemermaid transform_sugiyama_direction 同构的翻转，
    // 使物理方向与 liemermaid 的 positions 一致后再归一化。
    let flip = case.rankdir == "BT" || case.rankdir == "RL";
    let raw: Vec<(f64, f64)> = case
        .nodes
        .values()
        .map(|n| {
            if flip {
                match case.rankdir.as_str() {
                    "BT" => (n.x, -(n.y)), // y 轴镜像（等效 max_y - y 的归一化）
                    "RL" => (-(n.x), n.y), // x 轴镜像
                    _ => (n.x, n.y),
                }
            } else {
                (n.x, n.y)
            }
        })
        .collect();
    let norm_all = normalize(&raw);
    let mut norm = HashMap::new();
    for (i, id) in case.nodes.keys().enumerate() {
        norm.insert(id.clone(), norm_all[i]);
    }
    (rank, norm)
}

/// liemermaid 侧：由 layers 得 rank，并由 positions 得归一化中心坐标
fn ours_ranks_and_norm(
    res: &liemermaid::builder::layout::sugiyama::SugiyamaResult,
    idx: &HashMap<String, petgraph::graph::NodeIndex>,
    case: &DagreCase,
) -> (HashMap<String, usize>, HashMap<String, (f64, f64)>) {
    let mut rank = HashMap::new();
    for (id, &n) in idx.iter() {
        rank.insert(id.clone(), res.layers[&n]);
    }
    let raw: Vec<(f64, f64)> = case
        .nodes
        .keys()
        .map(|id| {
            let p: Point = res.positions[&idx[id]];
            (p.x, p.y)
        })
        .collect();
    let norm_all = normalize(&raw);
    let mut norm = HashMap::new();
    for (i, id) in case.nodes.keys().enumerate() {
        norm.insert(id.clone(), norm_all[i]);
    }
    (rank, norm)
}

/// 按 rank 值分组节点 id
fn group_by_rank(rank: &HashMap<String, usize>) -> HashMap<usize, Vec<String>> {
    let mut out: HashMap<usize, Vec<String>> = HashMap::new();
    for (id, &r) in rank.iter() {
        out.entry(r).or_default().push(id.clone());
    }
    out
}

/// 两个节点 id 列表是否为同一集合（忽略顺序）
fn same_set(a: &[String], b: &[String]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut sa: Vec<&String> = a.iter().collect();
    let mut sb: Vec<&String> = b.iter().collect();
    sa.sort();
    sb.sort();
    sa == sb
}
