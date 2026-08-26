use crate::ast::Flowchart;

/// 收集流程图中所有参与布局的节点（顶层 + 各 subgraph 内部，按 id 去重）。
///
/// 去重时合并节点信息：同一 id 在顶层可能是 edge 解析产生的空壳
/// （`shape: None, text: None`），而 subgraph 内部通常带有完整的 shape/text。
/// 因此当已存在节点为空壳、新节点有内容时，用有内容的覆盖。
pub fn all_flowchart_nodes(fc: &Flowchart) -> Vec<crate::ast::Node> {
    // 先按顶层节点顺序收集（保持布局入口的语义顺序），
    // 再把 subgraph 内部节点的 shape/text 合并进来补全空壳节点。
    let mut out: Vec<crate::ast::Node> = Vec::new();
    for node in &fc.nodes {
        if !out.iter().any(|e| e.id == node.id) {
            out.push(node.clone());
        }
    }

    // subgraph 内节点：仅合并信息到已存在节点，不新增（避免改变顺序/入口）
    for sg in &fc.subgraphs {
        for node in &sg.nodes {
            if let Some(existing) = out.iter_mut().find(|e| e.id == node.id) {
                // 空壳补全为有内容的节点
                if existing.shape.is_none() && node.shape.is_some() {
                    existing.shape = node.shape.clone();
                }
                if existing.text.is_none() && node.text.is_some() {
                    existing.text = node.text.clone();
                }
            } else {
                // subgraph 内存在、但顶层未出现过的节点（例如只在 subgraph 内声明）
                out.push(node.clone());
            }
        }
    }
    out
}
