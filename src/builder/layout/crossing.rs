//! 通用交叉减少原语 [`minimize_crossings`]（不依赖具体图 family / 方向）。
//!
//! 输入仅是一组分好层的节点序列 + 跨层边对；输出是层内顺序被重新排布、
//! 以最小化相邻层之间边交叉数的分层。barycenter（重心/中值）启发式，
//! 自上而下 + 自下而上多轮迭代。每轮基于「当前层内位置」**动态**计算 barycenter，
//! 避免静态邻接表在节点位置变化后错位。确定性由节点原始索引（插入序）做 tie-break 保证。

use std::collections::HashMap;

/// 节点 ID 类型（与 IR 同构）。
pub type NodeId = String;

/// 相邻层之间的边对（source 在前一层，target 在后一层）。
#[derive(Debug, Clone)]
pub struct LayerEdge {
    pub source: NodeId,
    pub target: NodeId,
}

/// 通用交叉减少：对 `layers`（index = 层号，按主布局方向从「首层」到「末层」排列）
/// 执行 barycenter 启发式，返回层内顺序被优化的新分层。
///
/// - `layers`：`Vec<Vec<NodeId>>`，第 i 个 Vec 是第 i 层的节点（顺序即当前排列）。
/// - `edges`：跨**相邻**层的边对（`source` 在层 k，`target` 在层 k+1）。
///   非相邻层边（如长边跨层）会被忽略（Sugiyama 长边通常会先被 dummy 节点化，
///   但本 IR 暂不引入 dummy，故只处理相邻层）。
///
/// 算法（每轮动态重算，保证位置一致）：
/// 1. 为每层构造 `node -> 当前 index` 映射。
/// 2. 偶数轮「自上而下」：对层 i (1..n)，按层 i-1 邻居的平均位置（barycenter）排序层 i。
/// 3. 奇数轮「自下而上」：对层 i (0..n-1)，按层 i+1 邻居的平均位置排序层 i。
/// 4. 重复 2/3 若干轮（默认 4 轮，足够收敛且确定性）。
///
/// 同 barycenter 用节点原始索引（在 `layers` 初始传入时的全局出现序）tie-break。
/// 返回的新分层与输入结构同形（节点集合不变，仅层内顺序变化）。
pub fn minimize_crossings(layers: &[Vec<NodeId>], edges: &[LayerEdge]) -> Vec<Vec<NodeId>> {
    if layers.is_empty() {
        return Vec::new();
    }
    let n_layers = layers.len();

    // 全局出现序：节点首次出现在 layers 中的索引（用于 tie-break，保证确定性）。
    let mut appearance: HashMap<&NodeId, usize> = HashMap::new();
    let mut seq = 0usize;
    for layer in layers {
        for id in layer {
            if !appearance.contains_key(id) {
                appearance.insert(id, seq);
                seq += 1;
            }
        }
    }

    // 工作副本
    let mut cur: Vec<Vec<NodeId>> = layers.to_vec();

    let rounds = 4;
    for r in 0..rounds {
        if r % 2 == 0 {
            // 自上而下
            for i in 1..n_layers {
                reorder_by_neighbors(&mut cur, i, i - 1, edges, &appearance);
            }
        } else {
            // 自下而上
            for i in (0..n_layers - 1).rev() {
                reorder_by_neighbors(&mut cur, i, i + 1, edges, &appearance);
            }
        }
    }

    cur
}

/// 依据「相邻层 `other_idx`」中邻居的位置，重排 `cur[layer_idx]` 层内顺序。
///
/// 对 `cur[layer_idx]` 中每个节点，收集其在 `cur[other_idx]` 里的邻居位置，
/// 计算 barycenter（平均），按 barycenter 升序排序；同分按 `appearance` 升序。
fn reorder_by_neighbors(
    cur: &mut [Vec<NodeId>],
    layer_idx: usize,
    other_idx: usize,
    edges: &[LayerEdge],
    appearance: &HashMap<&NodeId, usize>,
) {
    let n = cur[layer_idx].len();
    if n <= 1 {
        return;
    }
    // 当前两层内的位置映射
    let pos_in_layer: HashMap<&str, usize> = cur[layer_idx]
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let pos_in_other: HashMap<&str, usize> = cur[other_idx]
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    // 每个层内节点 -> 其在 other_idx 层的邻居位置列表
    let mut nb: HashMap<usize, Vec<usize>> = HashMap::new();
    for e in edges {
        let s = e.source.as_str();
        let t = e.target.as_str();
        // 正向：source 在 layer_idx，target 在 other_idx
        if let (Some(&sp), Some(&tp)) = (pos_in_layer.get(s), pos_in_other.get(t)) {
            nb.entry(sp).or_default().push(tp);
        }
        // 反向：source 在 other_idx，target 在 layer_idx
        if let (Some(&sp), Some(&tp)) = (pos_in_other.get(s), pos_in_layer.get(t)) {
            nb.entry(tp).or_default().push(sp);
        }
    }

    let layer = &cur[layer_idx];
    let mut bary: Vec<(usize, f64, usize)> = Vec::with_capacity(n);
    for (pos, id) in layer.iter().enumerate() {
        let mut sum = 0.0f64;
        let mut cnt = 0usize;
        if let Some(nbrs) = nb.get(&pos) {
            for &np in nbrs {
                sum += np as f64;
                cnt += 1;
            }
        }
        let b = if cnt > 0 { sum / cnt as f64 } else { pos as f64 };
        let app = *appearance.get(id).unwrap_or(&pos);
        bary.push((pos, b, app));
    }

    bary.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.2.cmp(&b.2))
    });

    let new_layer: Vec<NodeId> = bary.into_iter().map(|(pos, _, _)| layer[pos].clone()).collect();
    cur[layer_idx] = new_layer;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造简单分层 + 交叉边，验证 minimize_crossings 能消除交叉。
    #[test]
    fn reduces_a_simple_crossing() {
        // 层0: [A, B]   层1: [C, D]
        // 边 A->D, B->C  → 交叉。重排后层1应为 [D, C]，使边不交叉。
        let layers = vec![
            vec!["A".to_string(), "B".to_string()],
            vec!["C".to_string(), "D".to_string()],
        ];
        let edges = vec![
            LayerEdge { source: "A".to_string(), target: "D".to_string() },
            LayerEdge { source: "B".to_string(), target: "C".to_string() },
        ];
        let out = minimize_crossings(&layers, &edges);
        let crossings = count_crossings(&out, &edges);
        assert_eq!(crossings, 0, "minimize_crossings 应消除该简单交叉");
    }

    #[test]
    fn preserves_node_set() {
        let layers = vec![
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
            vec!["D".to_string(), "E".to_string()],
        ];
        let edges = vec![
            LayerEdge { source: "A".to_string(), target: "D".to_string() },
            LayerEdge { source: "B".to_string(), target: "E".to_string() },
            LayerEdge { source: "C".to_string(), target: "D".to_string() },
        ];
        let out = minimize_crossings(&layers, &edges);
        let mut flat: Vec<String> = out.iter().flat_map(|l| l.iter().cloned()).collect();
        flat.sort();
        let mut expect = ["A", "B", "C", "D", "E"].iter().map(|s| s.to_string()).collect::<Vec<_>>();
        expect.sort();
        assert_eq!(flat, expect);
    }

    /// 计数相邻层之间的边交叉数（不含共享端点）。
    fn count_crossings(layers: &[Vec<NodeId>], edges: &[LayerEdge]) -> usize {
        if layers.len() < 2 {
            return 0;
        }
        let pos = |li: usize, id: &str| -> Option<usize> { layers[li].iter().position(|x| x == id) };
        let mut count = 0;
        for li in 0..layers.len() - 1 {
            let mut segs: Vec<(usize, usize)> = Vec::new();
            for e in edges {
                if let (Some(sp), Some(tp)) = (pos(li, &e.source), pos(li + 1, &e.target)) {
                    segs.push((sp, tp));
                }
            }
            for i in 0..segs.len() {
                for j in i + 1..segs.len() {
                    let (s1, t1) = segs[i];
                    let (s2, t2) = segs[j];
                    if (s1 < s2 && t1 > t2) || (s1 > s2 && t1 < t2) {
                        count += 1;
                    }
                }
            }
        }
        count
    }
}
