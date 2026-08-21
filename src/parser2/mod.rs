//! `parser2`：基于 [winnow](https://docs.rs/winnow) 的 Mermaid 解析器（第二实现）。
//!
//! 与 [`crate::parser`]（pest 版）并存，复用 [`crate::ast`] 中的类型定义。
//! 目前实现了 flowchart 解析，后续将逐步补齐其它图表类型。
//!
//! # 用法
//! ```
//! use liemermaid::parser2::WinnowParser;
//!
//! let diagram = WinnowParser::parse_flowchart(
//!     "flowchart TD\nA[Start] --> B{Decision} --> C[End]",
//! )
//! .expect("parse failed");
//! assert_eq!(diagram.nodes.len(), 3);
//! assert_eq!(diagram.edges.len(), 2);
//! ```
//!
//! # 设计说明
//! - 输入流为 `&str`（完整输入，非 `Partial`），错误类型为 `ContextError`。
//! - 所有解析函数返回 `winnow::Result<T, ContextError>`（即 `Result<T, ErrMode<ContextError>>`）。
//! - 与 pest 版不同，winnow 版是**手写的组合式解析器**：逐条语句解析，
//!   支持行内注释、重复声明覆盖、引号 id 等。

pub mod flowchart;

use crate::ast::Flowchart;
use crate::error::{ParseError, ParseResult};
use winnow::error::ContextError;
use winnow::prelude::*;

/// winnow 版解析器入口。
///
/// 目前仅实现 flowchart；其它图表类型待补齐（届时会提供
/// [`MermaidParser::parse_mermaid`](crate::parser::MermaidParser::parse_mermaid)
/// 对应的统一入口）。
pub struct WinnowParser;

impl WinnowParser {
    /// 解析 flowchart 图表。
    ///
    /// 接受 `flowchart` 或 `graph` 开头的 Mermaid 文本。
    pub fn parse_flowchart(input: &str) -> ParseResult<Flowchart> {
        let mut stream: &str = input;
        let fc = flowchart::flowchart_diagram
            .parse_next(&mut stream)
            .map_err(|e| ParseError::Winnow(e.to_string()))?;
        // 剩余部分只允许空白/注释/换行
        let mut rest = stream;
        flowchart::trailing_ws
            .parse_next(&mut rest)
            .map_err(|e| ParseError::Winnow(e.to_string()))?;
        if !rest.is_empty() {
            return Err(ParseError::Winnow(format!(
                "trailing input not consumed: {:?}",
                &rest[..rest.len().min(40)]
            )));
        }
        Ok(fc)
    }
}

/// 便捷别名：winnow 解析错误（字符串化后的 ContextError）。
pub type WinnowResult<T> = Result<T, ContextError>;
