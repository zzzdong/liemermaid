//! state 图节点识别：收集参与布局/渲染的 state 节点元数据。
//!
//! 顺序与 `convert::ToLayoutGraph for StateDiagram` 的节点收集顺序严格一致
//! （显式 states 声明序 + transitions 补充序，按 id 去重），保证 `PlacedGraph.positions`
//! 与这里的节点列表一一对应。

use std::collections::{HashMap, HashSet};

use lievisual::geometry::Size;

use crate::ast::{State, StateDiagram};
use crate::builder::layout::ir::ShapeHint;

/// 单个 state 节点的布局/渲染元数据。
pub struct StateNodeInfo {
    /// 节点 ID（`__start__` / `__end__` 为 [*] 映射的特殊节点）。
    pub id: String,
    /// 显示文本（`State::Simple` 的 description），可能为 None。
    pub label: Option<String>,
    /// 形状类别。
    pub shape: ShapeHint,
    /// 节点包围盒尺寸（与 convert 一致）。
    pub size: Size,
}

/// 收集 state 图中所有参与布局的节点（顺序 = convert 的 `LayoutGraph.nodes` 顺序）。
pub fn collect_state_nodes(sd: &StateDiagram) -> Vec<StateNodeInfo> {
    // 显式 state 的显示文本映射（仅 Simple 有 description）。
    let mut labels: HashMap<String, Option<String>> = HashMap::new();
    for s in &sd.states {
        if let State::Simple { id, description } = s {
            labels.insert(id.clone(), description.clone());
        }
    }

    let mut out: Vec<StateNodeInfo> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let push = |out: &mut Vec<StateNodeInfo>,
                    seen: &mut HashSet<String>,
                    id: String,
                    label: Option<String>,
                    shape: ShapeHint,
                    size: Size| {
        if seen.contains(&id) {
            return;
        }
        seen.insert(id.clone());
        out.push(StateNodeInfo {
            id,
            label,
            shape,
            size,
        });
    };

    // 1. 显式 states 声明（Simple / Composite / Fork / Join / Start / End）
    for s in &sd.states {
        match s {
            State::Simple { id, .. } | State::Composite { id, .. } => {
                let label = labels.get(id).cloned().flatten();
                push(
                    &mut out,
                    &mut seen,
                    id.clone(),
                    label,
                    ShapeHint::Rect,
                    Size::new(100.0, 48.0),
                );
            }
            State::Fork { id } | State::Join { id } => {
                // fork / join：官方为水平横条。
                push(
                    &mut out,
                    &mut seen,
                    id.clone(),
                    None,
                    ShapeHint::Bar,
                    Size::new(100.0, 10.0),
                );
            }
            State::Start => {
                push(
                    &mut out,
                    &mut seen,
                    "__start__".into(),
                    None,
                    ShapeHint::Circle,
                    Size::new(32.0, 32.0),
                );
            }
            State::End => {
                push(
                    &mut out,
                    &mut seen,
                    "__end__".into(),
                    None,
                    ShapeHint::Circle,
                    Size::new(36.0, 36.0),
                );
            }
        }
    }

    // 2. transitions 补充出现的节点（states 为空时也覆盖；[*] 映射 start/end）
    for t in &sd.transitions {
        let (from, f_start): (&str, bool) = if t.from == "[*]" {
            ("__start__", true)
        } else {
            (&t.from, false)
        };
        let (to, t_end): (&str, bool) = if t.to == "[*]" {
            ("__end__", true)
        } else {
            (&t.to, false)
        };
        push(
            &mut out,
            &mut seen,
            from.to_string(),
            labels.get(from).cloned().flatten(),
            if f_start {
                ShapeHint::Circle
            } else {
                ShapeHint::Rect
            },
            if f_start {
                Size::new(32.0, 32.0)
            } else {
                Size::new(100.0, 48.0)
            },
        );
        push(
            &mut out,
            &mut seen,
            to.to_string(),
            labels.get(to).cloned().flatten(),
            if t_end {
                ShapeHint::Circle
            } else {
                ShapeHint::Rect
            },
            if t_end {
                Size::new(36.0, 36.0)
            } else {
                Size::new(100.0, 48.0)
            },
        );
    }

    out
}
