//! Stage 1: Extract —— AST 的唯一出口。
//!
//! 每个图类型一个 `extract_*` 文件，把 [`crate::ast::Diagram`] 的语义拓扑
//! 翻译成 [`crate::builder::ir::Unigraph`]。本阶段不碰任何坐标 / 尺寸 / 颜色。
//!
//! 进度：全部 8 种图类型已接入（flowchart / state / class / er / timeline / sequence / pie / gitgraph）。

pub mod class;
pub mod er;
pub mod flowchart;
pub mod gitgraph;
pub mod pie;
pub mod sequence;
pub mod state;
pub mod timeline;

use crate::builder::ir;

/// 从任意 Diagram 提取统一拓扑图（UG）。
///
/// 内部按 `diagram` 的图类型分派到具体 `extract_*` 实现。
/// 全部图类型均已接入，不再有降级分支。
pub fn run(diagram: &crate::ast::Diagram) -> crate::error::DiagramResult<ir::Unigraph> {
    match diagram {
        crate::ast::Diagram::Flowchart(fc) => {
            Ok(crate::builder::extract::flowchart::extract_flowchart(fc))
        }
        crate::ast::Diagram::State(sd) => Ok(crate::builder::extract::state::extract_state(sd)),
        crate::ast::Diagram::Class(cd) => Ok(crate::builder::extract::class::extract_class(cd)),
        crate::ast::Diagram::Er(ed) => Ok(crate::builder::extract::er::extract_er(ed)),
        crate::ast::Diagram::Timeline(td) => {
            Ok(crate::builder::extract::timeline::extract_timeline(td))
        }
        crate::ast::Diagram::Sequence(sd) => {
            Ok(crate::builder::extract::sequence::extract_sequence(sd))
        }
        crate::ast::Diagram::Pie(pd) => crate::builder::extract::pie::extract_pie(pd),
        crate::ast::Diagram::GitGraph(gd) => {
            Ok(crate::builder::extract::gitgraph::extract_gitgraph(gd))
        }
    }
}
