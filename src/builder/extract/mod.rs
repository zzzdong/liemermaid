//! Stage 1: Extract —— AST 的唯一出口。
//!
//! 每个图类型一个 `extract_*` 文件，把 [`crate::ast::Diagram`] 的语义拓扑
//! 翻译成 [`crate::builder::ir::Unigraph`]。本阶段不碰任何坐标 / 尺寸 / 颜色。
//!
//! 进度：P0.1 仅建骨架；`run` 将在 P0.3 实现（flowchart 最小子集先行）。

pub mod flowchart;

use crate::builder::ir;
use crate::error::DiagramError;

/// 从任意 Diagram 提取统一拓扑图（UG）。
///
/// 内部按 `diagram` 的图类型分派到具体 `extract_*` 实现。
/// P0.3 仅实现 flowchart；其余图类型在 P3 填充（当前返回错误）。
pub fn run(diagram: &crate::ast::Diagram) -> crate::error::DiagramResult<ir::Unigraph> {
    match diagram {
        crate::ast::Diagram::Flowchart(fc) => {
            Ok(crate::builder::extract::flowchart::extract_flowchart(fc))
        }
        other => Err(DiagramError::UnsupportedType(format!(
            "extract not yet implemented for {}",
            std::any::type_name_of_val(other)
        ))),
    }
}
