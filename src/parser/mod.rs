//! `parser2`：基于 [winnow](https://docs.rs/winnow) 的 Mermaid 解析器，是
//! [`crate::MermaidParser`] 的默认实现。
//!
//! 复用 [`crate::ast`] 中的类型定义，已实现对全部 8 种图表的解析：flowchart、
//! sequence、class、state、er、pie、timeline、gitgraph。
//!
//! # 用法
//! ```
//! use liemermaid::parser::WinnowParser;
//!
//! let diagram = WinnowParser::parse_mermaid(
//!     "flowchart TD\nA[Start]\nB{Decision}\nC[End]\nA --> B\nB --> C",
//! )
//! .expect("parse failed");
//! assert!(matches!(diagram, liemermaid::ast::Diagram::Flowchart(_)));
//! ```
//!
//! # 设计说明
//! - 输入流为 `&str`（完整输入，非 `Partial`），错误类型为 `ContextError`。
//! - winnow 版是**手写的组合式解析器**：逐条语句解析，支持行内注释、重复声明
//!   覆盖、引号 id 等。
//! - [`WinnowParser::parse_mermaid`] 依据首行头部关键字自动分派到对应图表解析器。

pub mod class;
pub mod common;
pub mod er;
pub mod flowchart;
pub mod gitgraph;
pub mod pie;
pub mod sequence;
pub mod state;
pub mod timeline;

use crate::ast::Diagram;
use crate::error::{ParseError, ParseResult};
use winnow::prelude::*;

/// winnow 版解析器入口。
pub struct WinnowParser;

impl WinnowParser {
    /// 统一解析入口：依据头部关键字自动识别图表类型。
    ///
    /// 支持 `flowchart`/`graph`、`sequenceDiagram`、`classDiagram`、
    /// `stateDiagram`/`stateDiagram-v2`、`erDiagram`、`pie`、`timeline`、`gitGraph`。
    pub fn parse_mermaid(input: &str) -> ParseResult<Diagram> {
        let trimmed = input.trim_start();
        let lower = trimmed.to_ascii_lowercase();
        // 关键字后须紧跟空白/行尾，避免误判前缀
        let kw = |p: &str| {
            lower
                .strip_prefix(p)
                .filter(|r| r.is_empty() || r.starts_with([' ', '\t', '\n', '\r', ':', '-']))
                .map(|r| &trimmed[trimmed.len() - r.len()..])
        };
        if kw("sequencediagram").is_some() {
            return Self::parse_sequence(trimmed);
        }
        if kw("classdiagram").is_some() {
            return Self::parse_class(trimmed);
        }
        if kw("statediagram-v2").is_some() {
            return Self::parse_state(trimmed);
        }
        if kw("statediagram").is_some() {
            return Self::parse_state(trimmed);
        }
        if kw("erdiagram").is_some() {
            return Self::parse_er(trimmed);
        }
        if kw("pie").is_some() {
            return Self::parse_pie(trimmed);
        }
        if kw("timeline").is_some() {
            return Self::parse_timeline(trimmed);
        }
        if kw("gitgraph").is_some() {
            return Self::parse_gitgraph(trimmed);
        }
        // flowchart/graph 与其他图表保持一致：大小写不敏感、去掉前导空白后传给解析器
        // （原实现用 `trimmed.starts_with` 区分大小写且把未 trim 的 input 传入，`Flowchart TD`
        // / 前导空格都会解析失败）。
        if kw("flowchart").is_some() || kw("graph").is_some() {
            return Self::parse_flowchart(trimmed);
        }
        Err(ParseError::UnsupportedDiagram)
    }

    /// 解析 flowchart 图表。
    pub fn parse_flowchart(input: &str) -> ParseResult<Diagram> {
        run(flowchart::flowchart_diagram, flowchart::trailing_ws, input).map(Diagram::Flowchart)
    }

    /// 解析 sequenceDiagram 图表。
    pub fn parse_sequence(input: &str) -> ParseResult<Diagram> {
        run(
            sequence::sequence_diagram,
            common::skip_ws_and_comments,
            input,
        )
        .map(Diagram::Sequence)
    }

    /// 解析 classDiagram 图表。
    pub fn parse_class(input: &str) -> ParseResult<Diagram> {
        run(class::class_diagram, common::skip_ws_and_comments, input).map(Diagram::Class)
    }

    /// 解析 stateDiagram / stateDiagram-v2 图表。
    pub fn parse_state(input: &str) -> ParseResult<Diagram> {
        run(state::state_diagram, common::skip_ws_and_comments, input).map(Diagram::State)
    }

    /// 解析 erDiagram 图表。
    pub fn parse_er(input: &str) -> ParseResult<Diagram> {
        run(er::er_diagram, common::skip_ws_and_comments, input).map(Diagram::Er)
    }

    /// 解析 pie 图表。
    pub fn parse_pie(input: &str) -> ParseResult<Diagram> {
        run(pie::pie_diagram, common::skip_ws_and_comments, input).map(Diagram::Pie)
    }

    /// 解析 timeline 图表。
    pub fn parse_timeline(input: &str) -> ParseResult<Diagram> {
        run(
            timeline::timeline_diagram,
            common::skip_ws_and_comments,
            input,
        )
        .map(Diagram::Timeline)
    }

    /// 解析 gitGraph 图表。
    pub fn parse_gitgraph(input: &str) -> ParseResult<Diagram> {
        run(
            gitgraph::gitgraph_diagram,
            common::skip_ws_and_comments,
            input,
        )
        .map(Diagram::GitGraph)
    }
}

/// 运行给定图表解析器，校验尾部仅含空白/注释。返回结构化错误。
fn run<'i, O>(
    mut parser: impl Parser<&'i str, O, winnow::error::InputError<&'i str>>,
    mut trailing: impl Parser<&'i str, (), winnow::error::InputError<&'i str>>,
    input: &'i str,
) -> ParseResult<O> {
    let mut stream: &str = input;
    let result = parser
        .parse_next(&mut stream)
        .map_err(|e: winnow::error::InputError<&str>| ParseError::Winnow(e.to_string()))?;
    let mut rest = stream;
    trailing
        .parse_next(&mut rest)
        .map_err(|e| ParseError::Winnow(e.to_string()))?;
    if !rest.is_empty() {
        return Err(ParseError::Winnow(format!(
            "trailing input? not consumed: {:?}",
            &rest[..rest.len().min(40)]
        )));
    }
    Ok(result)
}
