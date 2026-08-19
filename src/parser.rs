use pest::Parser;
use pest_derive::Parser;

use crate::ast::*;
use crate::error::{ParseError, ParseResult};

type Result<T> = ParseResult<T>;

#[derive(Parser)]
#[grammar = "grammar/mermaid.pest"]
pub struct MermaidParser;

impl MermaidParser {
    // ========== 公共解析入口 ==========
    pub fn parse_mermaid(input: &str) -> ParseResult<Diagram> {
        let parse_input = input;
        if std::env::var("LIEMERMAID_DEBUG_PARSE").is_ok() {
            eprintln!("parse_mermaid input: {:?}", parse_input);
        }
        match Self::parse(Rule::file, parse_input) {
            Ok(mut pairs) => {
                if std::env::var("LIEMERMAID_DEBUG_PARSE").is_ok() {
                    eprintln!("parse ok, pairs count: {}", pairs.len());
                }
                let file_pair = pairs.next().unwrap();
                let diagram_pair = file_pair
                    .into_inner()
                    .find(|p| {
                        matches!(
                            p.as_rule(),
                            Rule::flowchart_diagram
                                | Rule::sequence_diagram
                                | Rule::class_diagram
                                | Rule::state_diagram
                                | Rule::er_diagram
                                | Rule::pie_diagram
                                | Rule::timeline_diagram
                                | Rule::gg_diagram
                        )
                    })
                    .ok_or(ParseError::NoDiagram)?;
                Self::parse_diagram(diagram_pair)
            }
            Err(err) => {
                if std::env::var("LIEMERMAID_DEBUG_PARSE").is_ok() {
                    eprintln!("pest file parse failed: {:?}", err);
                    eprintln!(
                        "input len={}, bytes={:?}",
                        parse_input.len(),
                        parse_input.as_bytes()
                    );
                    match Self::parse(Rule::flowchart_diagram, parse_input) {
                        Ok(pairs) => eprintln!("rule=flowchart_diagram ok, pairs={}", pairs.len()),
                        Err(e2) => eprintln!("rule=flowchart_diagram failed: {:?}", e2),
                    }
                    match Self::parse(Rule::class_diagram, parse_input) {
                        Ok(pairs) => eprintln!("rule=class_diagram ok, pairs={}", pairs.len()),
                        Err(e2) => eprintln!("rule=class_diagram failed: {:?}", e2),
                    }
                }
                Err(ParseError::Pest(Box::new(err)))
            }
        }
    }

    // ========== 辅助提取函数 ==========
    fn pair_position(pair: &pest::iterators::Pair<Rule>) -> (usize, usize) {
        let (line, col) = pair.as_span().start_pos().line_col();
        (line, col)
    }

    fn invalid_syntax(pair: &pest::iterators::Pair<Rule>, msg: &str) -> ParseError {
        let (line, col) = Self::pair_position(pair);
        ParseError::InvalidSyntax {
            line,
            col,
            message: msg.to_string(),
        }
    }

    fn extract_string(pair: pest::iterators::Pair<Rule>) -> String {
        let s = pair.as_str();
        if pair.as_rule() == Rule::quoted_id
            || (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        {
            s[1..s.len() - 1].to_string()
        } else {
            s.to_string()
        }
    }

    fn extract_optional_string(pair: pest::iterators::Pair<Rule>, rule: Rule) -> Option<String> {
        pair.into_inner()
            .find(|p| p.as_rule() == rule)
            .map(Self::extract_string)
    }

    fn collect_strings(pair: pest::iterators::Pair<Rule>, rule: Rule) -> Vec<String> {
        pair.into_inner()
            .filter(|p| p.as_rule() == rule)
            .map(Self::extract_string)
            .collect()
    }

    // ========== 图表分发 ==========
    fn parse_diagram(pair: pest::iterators::Pair<Rule>) -> Result<Diagram> {
        match pair.as_rule() {
            Rule::flowchart_diagram => Ok(Diagram::Flowchart(Self::parse_flowchart(pair)?)),
            Rule::sequence_diagram => Ok(Diagram::Sequence(Self::parse_sequence(pair)?)),
            Rule::class_diagram => Ok(Diagram::Class(Self::parse_class(pair)?)),
            Rule::state_diagram => Ok(Diagram::State(Self::parse_state(pair)?)),
            Rule::er_diagram => Ok(Diagram::Er(Self::parse_er(pair)?)),
            Rule::pie_diagram => Ok(Diagram::Pie(Self::parse_pie(pair)?)),
            Rule::timeline_diagram => Ok(Diagram::Timeline(Self::parse_timeline(pair)?)),
            Rule::gg_diagram => Ok(Diagram::GitGraph(Self::parse_gitgraph(pair)?)),
            _ => Err(ParseError::UnsupportedDiagram),
        }
    }

    // ========== 流程图解析 ==========
    fn parse_flowchart(pair: pest::iterators::Pair<Rule>) -> Result<Flowchart> {
        let mut direction = None;
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut subgraphs = Vec::new();
        let mut node_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::direction => {
                    direction = Some(match inner.as_str() {
                        "TB" | "TD" => Direction::TD,
                        "BT" => Direction::BT,
                        "RL" => Direction::RL,
                        "LR" => Direction::LR,
                        _ => unreachable!(),
                    });
                }
                Rule::flowchart_statement => {
                    for stmt in inner.into_inner() {
                        match stmt.as_rule() {
                            Rule::node_decl => {
                                let node = Self::parse_node_decl(stmt)?;
                                if !node_ids.insert(node.id.clone()) {
                                    nodes.retain(|n: &Node| n.id != node.id);
                                }
                                nodes.push(node);
                            }
                            Rule::edge => {
                                for edge in Self::parse_edge(stmt)? {
                                    if node_ids.insert(edge.source.clone()) {
                                        nodes.push(Node {
                                            id: edge.source.clone(),
                                            shape: None,
                                            text: None,
                                        });
                                    }
                                    if node_ids.insert(edge.target.clone()) {
                                        nodes.push(Node {
                                            id: edge.target.clone(),
                                            shape: None,
                                            text: None,
                                        });
                                    }
                                    edges.push(edge);
                                }
                            }
                            Rule::subgraph => subgraphs.push(Self::parse_subgraph(stmt)?),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(Flowchart {
            direction,
            nodes,
            edges,
            subgraphs,
        })
    }

    fn parse_node_decl(pair: pest::iterators::Pair<Rule>) -> Result<Node> {
        let id = Self::extract_optional_string(pair.clone(), Rule::identifier)
            .ok_or_else(|| Self::invalid_syntax(&pair, "Node missing identifier"))?;

        let mut shape = None;
        let mut text = None;
        for inner in pair.into_inner() {
            if inner.as_rule() == Rule::node_shape {
                shape = Some(Self::parse_node_shape(&inner)?);
                // 递归搜索 node_text（可能嵌套在 node_shape → node_shape_brackets 内部）
                text = Self::find_node_text_recursive(&inner);
                break;
            }
        }
        Ok(Node { id, shape, text })
    }

    /// 递归搜索 parse 树中的 node_text
    fn find_node_text_recursive(pair: &pest::iterators::Pair<Rule>) -> Option<String> {
        for child in pair.clone().into_inner() {
            if child.as_rule() == Rule::node_text {
                return Self::extract_optional_string(child, Rule::text);
            }
            if let found @ Some(_) = Self::find_node_text_recursive(&child) {
                return found;
            }
        }
        None
    }

    fn parse_node_shape(pair: &pest::iterators::Pair<Rule>) -> Result<NodeShape> {
        let s = pair.as_str();
        let shape = match s {
            _ if s.starts_with("(((") => NodeShape::DoubleCircle,
            _ if s.starts_with("((") => NodeShape::Circle,
            _ if s.starts_with("([") => NodeShape::Stadium,
            _ if s.starts_with('(') && s.ends_with(')') => NodeShape::Rounded,
            _ if s.starts_with("[(") => NodeShape::Cylinder,
            _ if s.starts_with("[[") => NodeShape::Subroutine,
            _ if s.starts_with("[/") && s.ends_with("\\]") => NodeShape::Trapezoid,
            _ if s.starts_with("[/") && s.ends_with("/]") => NodeShape::Parallelogram,
            _ if s.starts_with("[\\") && s.ends_with("/]") => NodeShape::TrapezoidAlt,
            _ if s.starts_with("[\\") && s.ends_with("\\]") => NodeShape::ParallelogramAlt,
            _ if s.starts_with('[') && s.ends_with(']') => NodeShape::Rectangle,
            _ if s.starts_with("{{") => NodeShape::Hexagon,
            _ if s.starts_with('{') && s.ends_with('}') => NodeShape::Diamond,
            _ if s.starts_with('>') && s.ends_with(']') => NodeShape::Asymmetric,
            _ => NodeShape::Rectangle,
        };
        Ok(shape)
    }

    fn parse_edge(pair: pest::iterators::Pair<Rule>) -> Result<Vec<Edge>> {
        let pair_clone = pair.clone();
        let ids = Self::collect_strings(pair.clone(), Rule::identifier);
        if ids.len() < 2 {
            return Err(Self::invalid_syntax(&pair, "Edge missing source or target"));
        }

        // 每个箭头段 (source -> target) 对应一个 edge_arrow;按顺序配对 identifier。
        let mut arrows: Vec<(ArrowType, Option<String>)> = Vec::new();
        for inner in pair.into_inner() {
            if inner.as_rule() == Rule::edge_arrow {
                let mut arrow_type = Self::parse_edge_arrow(inner.clone())?;
                let mut label = None;
                for arrow_inner in inner.into_inner() {
                    match arrow_inner.as_rule() {
                        // -->|text| 形式
                        Rule::edge_arrow_labeled => {
                            if let Some(label_pair) = arrow_inner
                                .into_inner()
                                .find(|p| p.as_rule() == Rule::edge_label)
                            {
                                let label_text = label_pair.as_str().trim().to_string();
                                label = Some(label_text.clone());
                                arrow_type = ArrowType::Labeled(label_text);
                            }
                        }
                        // -- text --> / == text ==> / -. text .-> 形式
                        Rule::edge_arrow_solid_text
                        | Rule::edge_arrow_thick_text
                        | Rule::edge_arrow_dotted_text => {
                            let label_pair = arrow_inner.into_inner().find(|p| {
                                matches!(
                                    p.as_rule(),
                                    Rule::edge_text_solid
                                        | Rule::edge_text_thick
                                        | Rule::edge_text_dotted
                                )
                            });
                            if let Some(label_pair) = label_pair {
                                let label_text = label_pair.as_str().trim().to_string();
                                label = Some(label_text.clone());
                                arrow_type = ArrowType::Labeled(label_text);
                            }
                        }
                        _ => {}
                    }
                }
                arrows.push((arrow_type, label));
            }
        }

        if arrows.len() + 1 != ids.len() {
            return Err(Self::invalid_syntax(
                &pair_clone,
                "Edge identifier/arrow count mismatch",
            ));
        }

        let mut edges = Vec::with_capacity(arrows.len());
        for (i, (arrow_type, label)) in arrows.into_iter().enumerate() {
            edges.push(Edge {
                source: ids[i].clone(),
                target: ids[i + 1].clone(),
                arrow_type,
                label,
            });
        }
        Ok(edges)
    }

    fn parse_edge_arrow(pair: pest::iterators::Pair<Rule>) -> Result<ArrowType> {
        match pair.as_str() {
            "-->" => Ok(ArrowType::Solid),
            "-.->" => Ok(ArrowType::Dotted),
            "==>" => Ok(ArrowType::Thick),
            "---" => Ok(ArrowType::NoArrow),
            "--o" => Ok(ArrowType::Circle),
            "--x" => Ok(ArrowType::Cross),
            "<-->" => Ok(ArrowType::Both),
            _ => Ok(ArrowType::Solid), // 带标签的箭头也会匹配到这里，但标签由 edge_label 处理
        }
    }

    fn parse_subgraph(pair: pest::iterators::Pair<Rule>) -> Result<Subgraph> {
        let title = Self::extract_optional_string(pair.clone(), Rule::subgraph_title);
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::node_decl => nodes.push(Self::parse_node_decl(inner)?),
                Rule::edge => edges.extend(Self::parse_edge(inner)?),
                _ => {}
            }
        }

        Ok(Subgraph {
            title,
            nodes,
            edges,
        })
    }

    // ========== 时序图解析 ==========
    fn parse_sequence(pair: pest::iterators::Pair<Rule>) -> Result<SequenceDiagram> {
        let mut participants = Vec::new();
        let mut statements: Vec<SequenceStatement> = Vec::new();
        let mut participant_names: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for inner in pair.into_inner() {
            // Handle sequence_statement wrapper
            let actual_pair = if inner.as_rule() == Rule::sequence_statement {
                match inner.into_inner().next() {
                    Some(p) => p,
                    None => continue,
                }
            } else {
                inner
            };
            match actual_pair.as_rule() {
                Rule::participant => {
                    let kind = Self::participant_kind_of(&actual_pair);
                    let p = Self::parse_participant(actual_pair, kind)?;
                    participant_names.insert(p.name.clone());
                    participants.push(p);
                }
                Rule::message => {
                    let msg = Self::parse_message(actual_pair)?;
                    Self::register_message_participants(
                        &msg,
                        &mut participant_names,
                        &mut participants,
                    );
                    statements.push(SequenceStatement::Message(msg));
                }
                Rule::note => {
                    statements.push(SequenceStatement::Note(Self::parse_note(actual_pair)?))
                }
                Rule::seq_block => {
                    let ap_clone = actual_pair.clone();
                    let inner = actual_pair
                        .into_inner()
                        .next()
                        .ok_or_else(|| Self::invalid_syntax(&ap_clone, "空的分组块"))?;
                    let block = Self::parse_sequence_block(
                        inner,
                        &mut participant_names,
                        &mut participants,
                    )?;
                    statements.push(SequenceStatement::Block(block));
                }
                Rule::loop_block | Rule::alt_block | Rule::opt_block | Rule::par_block => {
                    let block = Self::parse_sequence_block(
                        actual_pair,
                        &mut participant_names,
                        &mut participants,
                    )?;
                    statements.push(SequenceStatement::Block(block));
                }
                _ => {}
            }
        }

        Ok(SequenceDiagram {
            participants,
            statements,
        })
    }

    fn register_message_participants(
        msg: &Message,
        participant_names: &mut std::collections::HashSet<String>,
        participants: &mut Vec<Participant>,
    ) {
        for name in [&msg.from, &msg.to] {
            if !participant_names.contains(name) {
                participant_names.insert(name.clone());
                participants.push(Participant {
                    name: name.clone(),
                    alias: None,
                    kind: ParticipantKind::Participant,
                });
            }
        }
    }

    /// 解析一个 sequence 块（loop/alt/opt/par），递归处理内部语句。
    /// participant_names/participants 会被内部消息自动注册（供后续列布局使用）。
    fn parse_sequence_block(
        pair: pest::iterators::Pair<Rule>,
        participant_names: &mut std::collections::HashSet<String>,
        participants: &mut Vec<Participant>,
    ) -> Result<SequenceBlock> {
        let rule = pair.as_rule();
        match rule {
            Rule::loop_block => {
                let label = Self::extract_optional_string(pair.clone(), Rule::seq_block_label)
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string());
                let items =
                    Self::parse_block_items(pair.into_inner(), participant_names, participants)?;
                Ok(SequenceBlock {
                    kind: SequenceBlockKind::Loop,
                    label,
                    items,
                })
            }
            Rule::opt_block => {
                let label = Self::extract_optional_string(pair.clone(), Rule::seq_block_label)
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string());
                let items =
                    Self::parse_block_items(pair.into_inner(), participant_names, participants)?;
                Ok(SequenceBlock {
                    kind: SequenceBlockKind::Opt,
                    label,
                    items,
                })
            }
            Rule::alt_block => Self::parse_alt_block(pair, participant_names, participants),
            Rule::par_block => Self::parse_par_block(pair, participant_names, participants),
            _ => Err(Self::invalid_syntax(&pair, "未知的分组块类型")),
        }
    }

    /// 处理块内部的一串语句（sequence_statement / 嵌套 seq_block），
    /// 忽略 else/and/end 等分隔字面量。
    fn parse_block_items(
        pairs: pest::iterators::Pairs<Rule>,
        participant_names: &mut std::collections::HashSet<String>,
        participants: &mut Vec<Participant>,
    ) -> Result<Vec<SequenceItem>> {
        let mut items: Vec<SequenceItem> = Vec::new();
        for inner in pairs {
            match inner.as_rule() {
                Rule::sequence_statement => {
                    let inner_clone = inner.clone();
                    let stmt = inner
                        .into_inner()
                        .next()
                        .ok_or_else(|| Self::invalid_syntax(&inner_clone, "空的分组语句"))?;
                    match stmt.as_rule() {
                        Rule::message => {
                            let msg = Self::parse_message(stmt)?;
                            Self::register_message_participants(
                                &msg,
                                participant_names,
                                participants,
                            );
                            items.push(SequenceItem::Message(msg));
                        }
                        Rule::note => items.push(SequenceItem::Note(Self::parse_note(stmt)?)),
                        Rule::loop_block | Rule::opt_block | Rule::par_block | Rule::alt_block => {
                            let b =
                                Self::parse_sequence_block(stmt, participant_names, participants)?;
                            items.push(SequenceItem::Block(b));
                        }
                        _ => {}
                    }
                }
                Rule::seq_block => {
                    let b = Self::parse_sequence_block(inner, participant_names, participants)?;
                    items.push(SequenceItem::Block(b));
                }
                _ => {} // else / and / end 等字面量忽略
            }
        }
        Ok(items)
    }

    /// 解析 alt 块：将 alt 主体 + 每个 else 分支拆分为独立的 Alt 块。
    /// 返回首个分支；若有多分支，则把后续分支作为首块 items 末尾的嵌套 Block（保持链式展示）。
    fn parse_alt_block(
        pair: pest::iterators::Pair<Rule>,
        participant_names: &mut std::collections::HashSet<String>,
        participants: &mut Vec<Participant>,
    ) -> Result<SequenceBlock> {
        let pair_clone = pair.clone();
        let mut branches: Vec<SequenceBlock> = Vec::new();
        let mut current_label: Option<String> = None;
        let mut current_items: Vec<SequenceItem> = Vec::new();
        let mut in_branch = false;

        for inner in pair.into_inner() {
            if let Rule::seq_alt_branch = inner.as_rule() {
                if in_branch {
                    branches.push(SequenceBlock {
                        kind: SequenceBlockKind::Alt,
                        label: current_label.take(),
                        items: std::mem::take(&mut current_items),
                    });
                }
                current_label = Self::extract_optional_string(inner.clone(), Rule::seq_block_label)
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string());
                current_items =
                    Self::parse_block_items(inner.into_inner(), participant_names, participants)?;
                in_branch = true;
            }
        }
        if in_branch {
            branches.push(SequenceBlock {
                kind: SequenceBlockKind::Alt,
                label: current_label.take(),
                items: current_items,
            });
        }

        let mut result = branches
            .clone()
            .into_iter()
            .next()
            .ok_or_else(|| Self::invalid_syntax(&pair_clone, "alt 块缺少分支"))?;
        // 把其余 else 分支作为末尾嵌套块，保证都能渲染
        for extra in branches.into_iter().skip(1) {
            result.items.push(SequenceItem::Block(extra));
        }
        Ok(result)
    }

    /// 解析 par 块：将 par 主体 + 每个 and 分支拆分为独立的 Par 块（与 alt 同构）。
    fn parse_par_block(
        pair: pest::iterators::Pair<Rule>,
        participant_names: &mut std::collections::HashSet<String>,
        participants: &mut Vec<Participant>,
    ) -> Result<SequenceBlock> {
        let pair_clone = pair.clone();
        let mut branches: Vec<SequenceBlock> = Vec::new();
        let mut current_label: Option<String> = None;
        let mut current_items: Vec<SequenceItem> = Vec::new();
        let mut in_branch = false;

        for inner in pair.into_inner() {
            if let Rule::seq_alt_branch = inner.as_rule() {
                if in_branch {
                    branches.push(SequenceBlock {
                        kind: SequenceBlockKind::Par,
                        label: current_label.take(),
                        items: std::mem::take(&mut current_items),
                    });
                }
                current_label = Self::extract_optional_string(inner.clone(), Rule::seq_block_label)
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string());
                current_items =
                    Self::parse_block_items(inner.into_inner(), participant_names, participants)?;
                in_branch = true;
            }
        }
        if in_branch {
            branches.push(SequenceBlock {
                kind: SequenceBlockKind::Par,
                label: current_label.take(),
                items: current_items,
            });
        }

        let mut result = branches
            .clone()
            .into_iter()
            .next()
            .ok_or_else(|| Self::invalid_syntax(&pair_clone, "par 块缺少分支"))?;
        for extra in branches.into_iter().skip(1) {
            result.items.push(SequenceItem::Block(extra));
        }
        Ok(result)
    }

    /// 从 participant 规则中提取参与者类型关键字对应的 [`ParticipantKind`]。
    fn participant_kind_of(pair: &pest::iterators::Pair<Rule>) -> ParticipantKind {
        let kw = pair
            .clone()
            .into_inner()
            .find(|p| p.as_rule() == Rule::participant_kind)
            .map(|p| p.as_str())
            .unwrap_or("participant");
        match kw {
            "actor" => ParticipantKind::Actor,
            "boundary" => ParticipantKind::Boundary,
            "control" => ParticipantKind::Control,
            "entity" => ParticipantKind::Entity,
            "database" => ParticipantKind::Database,
            "collections" => ParticipantKind::Collections,
            "queue" => ParticipantKind::Queue,
            _ => ParticipantKind::Participant,
        }
    }

    fn parse_participant(
        pair: pest::iterators::Pair<Rule>,
        kind: ParticipantKind,
    ) -> Result<Participant> {
        let ids = Self::collect_strings(pair.clone(), Rule::identifier);
        let name = ids
            .first()
            .ok_or_else(|| Self::invalid_syntax(&pair, "Participant missing name"))?
            .clone();
        let alias = ids.get(1).cloned();
        Ok(Participant { name, alias, kind })
    }

    fn parse_message(pair: pest::iterators::Pair<Rule>) -> Result<Message> {
        let ids = Self::collect_strings(pair.clone(), Rule::identifier);
        if ids.len() < 2 {
            return Err(Self::invalid_syntax(&pair, "Message missing from/to"));
        }
        let from = ids[0].clone();
        let to = ids[1].clone();

        let mut arrow = MessageArrow::Solid;
        let mut text = None;

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::message_arrow => arrow = Self::parse_message_arrow(inner)?,
                Rule::free_text => text = Some(inner.as_str().to_string()),
                _ => {}
            }
        }

        Ok(Message {
            from,
            to,
            arrow,
            text,
        })
    }

    fn parse_message_arrow(pair: pest::iterators::Pair<Rule>) -> Result<MessageArrow> {
        match pair.as_str() {
            "->" => Ok(MessageArrow::Solid),
            "->>" => Ok(MessageArrow::SolidTip),
            "-->" => Ok(MessageArrow::Dashed),
            "-->>" => Ok(MessageArrow::DashedTip),
            "-x" | "--x" => Ok(MessageArrow::Cross),
            "-)" | "--)" => Ok(MessageArrow::Open),
            "<<->>" | "<<-->>" => Ok(MessageArrow::Both),
            _ => Ok(MessageArrow::Solid),
        }
    }

    fn parse_note(pair: pest::iterators::Pair<Rule>) -> Result<Note> {
        let mut placement = NotePlacement::LeftOf;
        let mut targets = Vec::new();
        let mut text = String::new();

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::note_placement => {
                    placement = match inner.as_str().trim() {
                        s if s.starts_with("left") => NotePlacement::LeftOf,
                        s if s.starts_with("right") => NotePlacement::RightOf,
                        s if s.starts_with("over") => NotePlacement::Over,
                        _ => NotePlacement::Over,
                    };
                }
                Rule::note_target => {
                    targets = Self::collect_strings(inner, Rule::identifier);
                }
                Rule::free_text => text = inner.as_str().to_string(),
                _ => {}
            }
        }

        Ok(Note {
            placement,
            targets,
            text,
        })
    }

    // ========== 类图解析 ==========
    fn parse_class(pair: pest::iterators::Pair<Rule>) -> Result<ClassDiagram> {
        let mut classes = Vec::new();
        let mut relations = Vec::new();

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::class_decl => classes.push(Self::parse_class_decl(inner)?),
                Rule::class_member_decl => {
                    let ids = Self::collect_strings(inner, Rule::identifier);
                    if ids.len() >= 2 {
                        let class_name = ids[0].clone();
                        let member_type = ids[1].clone();
                        // Find or create the class
                        if let Some(class) = classes
                            .iter_mut()
                            .find(|c: &&mut Class| c.name == class_name)
                        {
                            class.members.push(ClassMember {
                                visibility: None,
                                name: member_type.clone(),
                                type_: None,
                                is_method: false,
                            });
                        } else {
                            classes.push(Class {
                                name: class_name,
                                members: vec![ClassMember {
                                    visibility: None,
                                    name: member_type,
                                    type_: None,
                                    is_method: false,
                                }],
                            });
                        }
                    }
                }
                Rule::relation => relations.push(Self::parse_relation(inner)?),
                _ => {}
            }
        }

        Ok(ClassDiagram { classes, relations })
    }

    fn parse_class_decl(pair: pest::iterators::Pair<Rule>) -> Result<Class> {
        let name = Self::extract_optional_string(pair.clone(), Rule::identifier)
            .ok_or_else(|| Self::invalid_syntax(&pair, "Class missing name"))?;
        let members = Self::collect_class_members(pair);
        Ok(Class { name, members })
    }

    fn collect_class_members(pair: pest::iterators::Pair<Rule>) -> Vec<ClassMember> {
        let mut members = Vec::new();
        for inner in pair.into_inner() {
            if inner.as_rule() == Rule::class_members {
                for member_pair in inner.into_inner() {
                    if member_pair.as_rule() == Rule::class_member
                        && let Ok(m) = Self::parse_class_member(member_pair)
                    {
                        members.push(m);
                    }
                }
            }
        }
        members
    }

    fn parse_class_member(pair: pest::iterators::Pair<Rule>) -> Result<ClassMember> {
        let mut visibility = None;
        let mut name = None;
        let mut type_ = None;
        let err_pair = pair.clone();
        let full_str = pair.as_str();

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::visibility => {
                    visibility = Some(match inner.as_str() {
                        "+" => Visibility::Public,
                        "-" => Visibility::Private,
                        "#" => Visibility::Protected,
                        "~" => Visibility::Package,
                        _ => Visibility::Public,
                    });
                }
                Rule::identifier => {
                    if name.is_none() {
                        name = Some(Self::extract_string(inner));
                    } else {
                        type_ = Some(Self::extract_string(inner));
                    }
                }
                _ => {}
            }
        }

        let name =
            name.ok_or_else(|| Self::invalid_syntax(&err_pair, "Class member missing name"))?;
        let is_method = full_str.contains('(');

        Ok(ClassMember {
            visibility,
            name,
            type_,
            is_method,
        })
    }

    fn parse_relation(pair: pest::iterators::Pair<Rule>) -> Result<Relation> {
        let ids = Self::collect_strings(pair.clone(), Rule::identifier);
        if ids.len() < 2 {
            return Err(Self::invalid_syntax(
                &pair,
                "Relation missing source or target",
            ));
        }
        let source = ids[0].clone();
        let target = ids[1].clone();

        let mut kind = RelationKind::Association;
        let mut label = None;

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::relation_type => {
                    kind = match inner.as_str() {
                        "<|--" => RelationKind::Inheritance,
                        "*--" => RelationKind::Composition,
                        "o--" => RelationKind::Aggregation,
                        "-->" => RelationKind::Association,
                        "..>" => RelationKind::Dependency,
                        _ => RelationKind::Association,
                    };
                }
                Rule::free_text => label = Some(inner.as_str().to_string()),
                _ => {}
            }
        }

        Ok(Relation {
            source,
            target,
            kind,
            label,
        })
    }

    // ========== 状态图解析 ==========
    fn parse_state(pair: pest::iterators::Pair<Rule>) -> Result<StateDiagram> {
        let mut states = Vec::new();
        let mut transitions = Vec::new();

        for inner in pair.into_inner() {
            // Handle state_element wrapper
            let actual_pair = if inner.as_rule() == Rule::state_element {
                match inner.into_inner().next() {
                    Some(p) => p,
                    None => continue,
                }
            } else {
                inner
            };
            match actual_pair.as_rule() {
                Rule::state_simple => states.push(Self::parse_state_simple(actual_pair)?),
                Rule::state_composite => states.push(Self::parse_state_composite(actual_pair)?),
                Rule::transition => transitions.push(Self::parse_transition(actual_pair)?),
                _ => {}
            }
        }

        Ok(StateDiagram {
            states,
            transitions,
        })
    }

    fn parse_state_simple(pair: pest::iterators::Pair<Rule>) -> Result<State> {
        let id = Self::extract_optional_string(pair.clone(), Rule::identifier)
            .or_else(|| Self::extract_optional_string(pair.clone(), Rule::state_id))
            .ok_or_else(|| Self::invalid_syntax(&pair, "State missing id"))?;
        let description = Self::extract_optional_string(pair, Rule::free_text);
        Ok(State::Simple { id, description })
    }

    fn parse_state_composite(pair: pest::iterators::Pair<Rule>) -> Result<State> {
        let id = Self::extract_optional_string(pair.clone(), Rule::identifier)
            .or_else(|| Self::extract_optional_string(pair.clone(), Rule::state_id))
            .ok_or_else(|| Self::invalid_syntax(&pair, "Composite state missing id"))?;
        // 递归解析内部子图（注意：pair 中内部内容仍然是 state_diagram 规则）
        let inner_diagram = Self::parse_state(pair)?;
        Ok(State::Composite {
            id,
            inner: Box::new(inner_diagram),
        })
    }

    fn parse_transition(pair: pest::iterators::Pair<Rule>) -> Result<Transition> {
        let mut from = None;
        let mut to = None;
        let mut label = None;
        let err_pair = pair.clone();

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::identifier => {
                    let id = Self::extract_string(inner);
                    if from.is_none() {
                        from = Some(id);
                    } else {
                        to = Some(id);
                    }
                }
                Rule::state_id => {
                    let id = Self::extract_string(inner);
                    if from.is_none() {
                        from = Some(id);
                    } else {
                        to = Some(id);
                    }
                }
                Rule::start_end_state => {
                    if from.is_none() {
                        from = Some("[*]".to_string());
                    } else {
                        to = Some("[*]".to_string());
                    }
                }
                _ => {
                    if inner.as_rule() == Rule::free_text {
                        label = Some(inner.as_str().to_string());
                    }
                }
            }
        }

        let from =
            from.ok_or_else(|| Self::invalid_syntax(&err_pair, "Transition missing 'from' state"))?;
        let to =
            to.ok_or_else(|| Self::invalid_syntax(&err_pair, "Transition missing 'to' state"))?;

        Ok(Transition { from, to, label })
    }

    // ========== ER图解析 ==========
    fn parse_er(pair: pest::iterators::Pair<Rule>) -> Result<ErDiagram> {
        let mut entities = Vec::new();
        let mut relationships = Vec::new();

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::entity => entities.push(Self::parse_er_entity(inner)?),
                Rule::er_statement => relationships.push(Self::parse_er_statement(inner)?),
                _ => {}
            }
        }

        Ok(ErDiagram {
            entities,
            relationships,
        })
    }

    fn parse_er_entity(pair: pest::iterators::Pair<Rule>) -> Result<ErEntity> {
        let name = Self::extract_optional_string(pair.clone(), Rule::identifier)
            .ok_or_else(|| Self::invalid_syntax(&pair, "Entity missing name"))?;
        let attributes = Self::collect_attributes(pair);
        Ok(ErEntity { name, attributes })
    }

    fn collect_attributes(pair: pest::iterators::Pair<Rule>) -> Vec<ErAttribute> {
        let mut attrs = Vec::new();
        for inner in pair.into_inner() {
            if inner.as_rule() == Rule::attribute
                && let Ok(attr) = Self::parse_attribute(inner)
            {
                attrs.push(attr);
            }
        }
        attrs
    }

    fn parse_attribute(pair: pest::iterators::Pair<Rule>) -> Result<ErAttribute> {
        let ids = Self::collect_strings(pair.clone(), Rule::identifier);
        if ids.len() < 2 {
            return Err(Self::invalid_syntax(
                &pair,
                "Attribute missing type or name",
            ));
        }
        Ok(ErAttribute {
            type_: ids[0].clone(),
            name: ids[1].clone(),
        })
    }

    fn parse_er_statement(pair: pest::iterators::Pair<Rule>) -> Result<ErRelationship> {
        let ids = Self::collect_strings(pair.clone(), Rule::identifier);
        if ids.len() < 2 {
            return Err(Self::invalid_syntax(&pair, "Relationship missing entities"));
        }
        let first_entity = ids[0].clone();
        let second_entity = ids[1].clone();

        let mut cardinality_first = Cardinality::ZeroOrOne;
        let mut cardinality_second = Cardinality::ZeroOrOne;
        let mut label = None;

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::relationship => {
                    let cards: Vec<Cardinality> = inner
                        .into_inner()
                        .filter(|c| c.as_rule() == Rule::cardinality)
                        .map(|c| match c.as_str() {
                            "|o" => Cardinality::ZeroOrOne,
                            "||" => Cardinality::ExactlyOne,
                            "}o" => Cardinality::ZeroOrMany,
                            "}|" => Cardinality::OneOrMany,
                            _ => Cardinality::ZeroOrOne,
                        })
                        .collect();
                    if cards.len() >= 2 {
                        cardinality_first = cards[0].clone();
                        cardinality_second = cards[1].clone();
                    }
                }
                Rule::free_text => label = Some(inner.as_str().to_string()),
                _ => {}
            }
        }

        Ok(ErRelationship {
            first_entity,
            second_entity,
            cardinality_first,
            cardinality_second,
            label,
        })
    }

    // ========== 饼图解析 ==========
    fn parse_pie(pair: pest::iterators::Pair<Rule>) -> Result<PieDiagram> {
        let mut title = None;
        let mut show_data = false;
        let mut data = Vec::new();

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::pie_modifier => {
                    let s = inner.as_str();
                    if let Some(stripped) = s.strip_prefix("title ") {
                        title = Some(stripped.to_string());
                    } else if s == "showData" {
                        show_data = true;
                    }
                }
                Rule::pie_data_entry => {
                    let parts: Vec<&str> = inner.as_str().splitn(2, ':').collect();
                    if parts.len() == 2 {
                        data.push(PieData {
                            label: parts[0].trim().to_string(),
                            value: parts[1].trim().to_string(),
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(PieDiagram {
            title,
            show_data,
            data,
        })
    }

    // ========== 时间线解析 ==========
    fn parse_timeline(pair: pest::iterators::Pair<Rule>) -> Result<TimelineDiagram> {
        let mut title = None;
        let mut sections = Vec::new();
        let mut current_section: Option<String> = None;

        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::tl_title => {
                    let s = inner.as_str();
                    // 关键字可能是 Title/title（grammar 已大小写不敏感），去掉前缀取标签文本
                    let stripped = s
                        .strip_prefix("title ")
                        .or_else(|| s.strip_prefix("Title "))
                        .or_else(|| s.strip_prefix("TITLE "))
                        .unwrap_or(s);
                    let text = stripped.trim();
                    if !text.is_empty() {
                        title = Some(text.to_string());
                    }
                }
                Rule::tl_section => {
                    let s = inner.as_str();
                    if let Some(stripped) = s.strip_prefix("section ") {
                        let name = stripped.to_string();
                        current_section = Some(name);
                    }
                }
                Rule::tl_event_line => {
                    let s = inner.as_str();
                    let parts: Vec<&str> = s.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let section_name = current_section.clone().unwrap_or_default();
                        let event = parts[1].trim().to_string();
                        if let Some(section) = sections
                            .iter_mut()
                            .find(|sec: &&mut TimelineSection| sec.name == section_name)
                        {
                            section.events.push(event);
                        } else {
                            sections.push(TimelineSection {
                                name: section_name,
                                events: vec![event],
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(TimelineDiagram { title, sections })
    }

    // ========== Git 分支图解析 ==========
    fn parse_gitgraph(pair: pest::iterators::Pair<Rule>) -> Result<GitGraphDiagram> {
        let mut statements = Vec::new();

        for inner in pair.into_inner() {
            if inner.as_rule() == Rule::gg_statement {
                for stmt in inner.into_inner() {
                    match stmt.as_rule() {
                        Rule::gg_commit => {
                            let tag =
                                stmt.into_inner()
                                    .find(|p| p.as_rule() == Rule::gg_tag)
                                    .map(|p| {
                                        let s = p.as_str();
                                        if let Some(stripped) = s.strip_prefix("tag:") {
                                            stripped.trim().to_string()
                                        } else {
                                            s.to_string()
                                        }
                                    });
                            statements.push(GitGraphStatement::Commit { tag });
                        }
                        Rule::gg_branch => {
                            let name = Self::extract_optional_string(stmt, Rule::identifier)
                                .unwrap_or_default();
                            statements.push(GitGraphStatement::Branch { name });
                        }
                        Rule::gg_checkout => {
                            let branch = Self::extract_optional_string(stmt, Rule::identifier)
                                .unwrap_or_default();
                            statements.push(GitGraphStatement::Checkout { branch });
                        }
                        Rule::gg_merge => {
                            let branch = Self::extract_optional_string(stmt, Rule::identifier)
                                .unwrap_or_default();
                            statements.push(GitGraphStatement::Merge { branch });
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(GitGraphDiagram { statements })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_flowchart() {
        let input = "flowchart TD\nA --> B";
        let diagram = MermaidParser::parse_mermaid(input).unwrap();
        match diagram {
            Diagram::Flowchart(flowchart) => {
                assert_eq!(flowchart.direction, Some(Direction::TD));
                assert_eq!(flowchart.nodes.len(), 2);
                assert_eq!(flowchart.edges.len(), 1);
                assert_eq!(flowchart.edges[0].source, "A");
                assert_eq!(flowchart.edges[0].target, "B");
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_graph_keyword() {
        let input = "graph LR\nA --> B";
        let diagram = MermaidParser::parse_mermaid(input).unwrap();
        match diagram {
            Diagram::Flowchart(flowchart) => {
                assert_eq!(flowchart.direction, Some(Direction::LR));
                assert_eq!(flowchart.edges.len(), 1);
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_chained_edges() {
        let input = "flowchart TD\nA --> B --> C --> D";
        let diagram = MermaidParser::parse_mermaid(input).unwrap();
        match diagram {
            Diagram::Flowchart(flowchart) => {
                assert_eq!(flowchart.nodes.len(), 4);
                assert_eq!(flowchart.edges.len(), 3);
                assert_eq!(flowchart.edges[0].source, "A");
                assert_eq!(flowchart.edges[0].target, "B");
                assert_eq!(flowchart.edges[1].source, "B");
                assert_eq!(flowchart.edges[1].target, "C");
                assert_eq!(flowchart.edges[2].source, "C");
                assert_eq!(flowchart.edges[2].target, "D");
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_no_direction() {
        let input = "flowchart\nA --> B";
        let diagram = MermaidParser::parse_mermaid(input).unwrap();
        match diagram {
            Diagram::Flowchart(flowchart) => {
                assert_eq!(flowchart.direction, None);
                assert_eq!(flowchart.edges.len(), 1);
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_dotted_edge() {
        let input = "flowchart LR\nA -.-> B";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(flowchart) => {
                assert_eq!(flowchart.edges[0].arrow_type, ArrowType::Dotted);
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_thick_edge() {
        let input = "flowchart LR\nA ==> B";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(flowchart) => {
                assert_eq!(flowchart.edges[0].arrow_type, ArrowType::Thick);
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_labeled_edge() {
        let input = "flowchart TD\nA -->|hello| B";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(flowchart) => {
                assert_eq!(flowchart.edges.len(), 1);
                assert_eq!(flowchart.edges[0].label.as_deref(), Some("hello"));
                assert_eq!(
                    flowchart.edges[0].arrow_type,
                    ArrowType::Labeled("hello".to_string())
                );
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_inline_text_edges() {
        // -- text --> / == text ==> / -. text .-> 内联文本边
        let input = "flowchart TD\nA -- yes --> B\nB == strong ==> C\nC -. dotted .-> A";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(flowchart) => {
                assert_eq!(flowchart.edges.len(), 3);
                assert_eq!(flowchart.edges[0].label.as_deref(), Some("yes"));
                assert_eq!(
                    flowchart.edges[0].arrow_type,
                    ArrowType::Labeled("yes".to_string())
                );
                assert_eq!(flowchart.edges[1].label.as_deref(), Some("strong"));
                assert_eq!(
                    flowchart.edges[1].arrow_type,
                    ArrowType::Labeled("strong".to_string())
                );
                assert_eq!(flowchart.edges[2].label.as_deref(), Some("dotted"));
                assert_eq!(
                    flowchart.edges[2].arrow_type,
                    ArrowType::Labeled("dotted".to_string())
                );
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_inline_text_no_spaces() {
        // 无空格紧凑写法：A--text-->B
        let input = "flowchart LR\nA--go-->B";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(flowchart) => {
                assert_eq!(flowchart.edges.len(), 1);
                assert_eq!(flowchart.edges[0].source, "A");
                assert_eq!(flowchart.edges[0].target, "B");
                assert_eq!(flowchart.edges[0].label.as_deref(), Some("go"));
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_multiple_edges() {
        let input = "flowchart TD\nA --> B\nB --> C\nC --> D";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(fc) => {
                assert_eq!(fc.nodes.len(), 4);
                assert_eq!(fc.edges.len(), 3);
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_no_arrow_edge() {
        let input = "flowchart LR\nA --- B";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(fc) => {
                assert_eq!(fc.edges.len(), 1);
                assert_eq!(fc.edges[0].arrow_type, ArrowType::NoArrow);
                assert_eq!(fc.edges[0].source, "A");
                assert_eq!(fc.edges[0].target, "B");
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_both_arrow_edge() {
        let input = "flowchart LR\nA <--> B";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(fc) => {
                assert_eq!(fc.edges.len(), 1);
                assert_eq!(fc.edges[0].arrow_type, ArrowType::Both);
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_no_arrow_does_not_swallow_solid() {
        // 确保 --- 不会破坏已有的 --> 解析
        let input = "flowchart LR\nA --> B\nC --- D";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(fc) => {
                assert_eq!(fc.edges.len(), 2);
                assert_eq!(fc.edges[0].arrow_type, ArrowType::Solid);
                assert_eq!(fc.edges[1].arrow_type, ArrowType::NoArrow);
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_circle_and_cross_edges() {
        let input = "flowchart LR\nA --o B\nC --x D";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(fc) => {
                assert_eq!(fc.edges.len(), 2);
                assert_eq!(fc.edges[0].arrow_type, ArrowType::Circle);
                assert_eq!(fc.edges[1].arrow_type, ArrowType::Cross);
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_circle_cross_no_swallow() {
        // --o / --x 不应破坏 --> 与 ---
        let input = "flowchart LR\nA --> B\nC --- D\nE --o F\nG --x H";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(fc) => {
                assert_eq!(fc.edges.len(), 4);
                assert_eq!(fc.edges[0].arrow_type, ArrowType::Solid);
                assert_eq!(fc.edges[1].arrow_type, ArrowType::NoArrow);
                assert_eq!(fc.edges[2].arrow_type, ArrowType::Circle);
                assert_eq!(fc.edges[3].arrow_type, ArrowType::Cross);
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_node_shapes() {
        let input = "flowchart TD\nA[Start]\nA --> B\nB{Decision}\nB --> C\nC[End]";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(fc) => {
                assert_eq!(fc.nodes.len(), 3);
                assert_eq!(fc.nodes[0].shape, Some(NodeShape::Rectangle));
                assert_eq!(fc.nodes[1].shape, Some(NodeShape::Diamond));
                assert_eq!(fc.nodes[2].shape, Some(NodeShape::Rectangle));
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_quoted_ids() {
        let input = r#"flowchart TD
"A" --> "B""#;
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(fc) => {
                assert_eq!(fc.edges[0].source, "A");
                assert_eq!(fc.edges[0].target, "B");
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_simple_sequence_diagram() {
        let input = "sequenceDiagram\nalice->>bob: Hi";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Sequence(seq) => {
                assert_eq!(seq.participants.len(), 2);
                assert_eq!(seq.statements.len(), 1);
                match &seq.statements[0] {
                    SequenceStatement::Message(m) => assert_eq!(m.text.as_deref(), Some("Hi")),
                    _ => panic!("expected Message"),
                }
            }
            _ => panic!("expected Sequence"),
        }
    }

    #[test]
    fn parses_sequence_with_participants() {
        let input = "sequenceDiagram\nparticipant Alice\nparticipant Bob as B\nalice->>bob: Hello";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Sequence(seq) => {
                assert_eq!(seq.participants.len(), 4);
                assert!(seq.participants.iter().any(|p| p.name == "Alice"));
                assert!(seq.participants.iter().any(|p| p.name == "bob"));
            }
            _ => panic!("expected Sequence"),
        }
    }

    #[test]
    fn parses_sequence_participant_kinds() {
        let input = "sequenceDiagram\nparticipant A\nactor B\nboundary C\ncontrol D\nentity E\ndatabase F\ncollections G\nqueue H";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Sequence(seq) => {
                let kinds: Vec<ParticipantKind> = seq
                    .participants
                    .iter()
                    .map(|p| p.kind)
                    .collect();
                assert_eq!(
                    kinds,
                    vec![
                        ParticipantKind::Participant,
                        ParticipantKind::Actor,
                        ParticipantKind::Boundary,
                        ParticipantKind::Control,
                        ParticipantKind::Entity,
                        ParticipantKind::Database,
                        ParticipantKind::Collections,
                        ParticipantKind::Queue,
                    ]
                );
            }
            _ => panic!("expected Sequence"),
        }
    }

    #[test]
    fn parses_sequence_both_arrows() {
        let input = "sequenceDiagram\nA <<->> B\nC <<-->> D";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Sequence(seq) => {
                assert_eq!(seq.statements.len(), 2);
                match &seq.statements[0] {
                    SequenceStatement::Message(m) => assert_eq!(m.arrow, MessageArrow::Both),
                    _ => panic!("expected Message"),
                }
                match &seq.statements[1] {
                    SequenceStatement::Message(m) => assert_eq!(m.arrow, MessageArrow::Both),
                    _ => panic!("expected Message"),
                }
            }
            _ => panic!("expected Sequence"),
        }
    }

    #[test]
    fn parses_sequence_dashed_cross_open_arrows() {
        let input = "sequenceDiagram\nA --x B\nC --) D";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Sequence(seq) => {
                assert_eq!(seq.statements.len(), 2);
                match &seq.statements[0] {
                    SequenceStatement::Message(m) => assert_eq!(m.arrow, MessageArrow::Cross),
                    _ => panic!("expected Message"),
                }
                match &seq.statements[1] {
                    SequenceStatement::Message(m) => assert_eq!(m.arrow, MessageArrow::Open),
                    _ => panic!("expected Message"),
                }
            }
            _ => panic!("expected Sequence"),
        }
    }

    #[test]
    fn parses_sequence_with_note() {
        let input = "sequenceDiagram\nalice->>bob: Hello\nnote over alice,bob: A note";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Sequence(seq) => {
                assert_eq!(seq.statements.len(), 2);
                match &seq.statements[1] {
                    SequenceStatement::Note(n) => {
                        assert_eq!(n.text, "A note");
                        assert_eq!(n.placement, NotePlacement::Over);
                    }
                    _ => panic!("expected Note"),
                }
            }
            _ => panic!("expected Sequence"),
        }
    }

    #[test]
    fn parses_sequence_loop_block() {
        let input = "sequenceDiagram\nA->>B: hi\nloop every minute\nB->>C: tick\nend";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Sequence(seq) => {
                assert_eq!(seq.statements.len(), 2);
                match &seq.statements[1] {
                    SequenceStatement::Block(b) => {
                        assert_eq!(b.kind, SequenceBlockKind::Loop);
                        assert_eq!(b.label.as_deref(), Some("every minute"));
                        assert_eq!(b.items.len(), 1);
                        match &b.items[0] {
                            SequenceItem::Message(m) => assert_eq!(m.text.as_deref(), Some("tick")),
                            _ => panic!("expected inner message"),
                        }
                    }
                    _ => panic!("expected Block"),
                }
            }
            _ => panic!("expected Sequence"),
        }
    }

    #[test]
    fn parses_sequence_alt_block_with_else() {
        let input =
            "sequenceDiagram\nA->>B: hi\nalt is ok\nB->>C: go\nelse is bad\nB->>D: stop\nend";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Sequence(seq) => {
                assert_eq!(seq.statements.len(), 2);
                match &seq.statements[1] {
                    SequenceStatement::Block(b) => {
                        assert_eq!(b.kind, SequenceBlockKind::Alt);
                        assert_eq!(b.label.as_deref(), Some("is ok"));
                        // 首个分支的消息 + 末尾追加的 else 嵌套块
                        assert!(b.items.len() >= 2);
                        assert!(matches!(&b.items[0], SequenceItem::Message(_)));
                        // 检查是否存在嵌套的 else 分支块
                        let has_else = b
                            .items
                            .iter()
                            .any(|i| matches!(i, SequenceItem::Block(ib) if ib.kind == SequenceBlockKind::Alt));
                        assert!(has_else, "alt 的 else 分支应被解析为嵌套块");
                    }
                    _ => panic!("expected Block"),
                }
            }
            _ => panic!("expected Sequence"),
        }
    }

    #[test]
    fn parses_sequence_opt_and_par_blocks() {
        let input = "sequenceDiagram\nopt caching\nA->>B: get\nend\npar step1\nA->>C: one\nand step2\nA->>D: two\nend";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Sequence(seq) => {
                assert_eq!(seq.statements.len(), 2);
                match &seq.statements[0] {
                    SequenceStatement::Block(b) => {
                        assert_eq!(b.kind, SequenceBlockKind::Opt);
                        assert_eq!(b.label.as_deref(), Some("caching"));
                    }
                    _ => panic!("expected opt Block"),
                }
                match &seq.statements[1] {
                    SequenceStatement::Block(b) => {
                        assert_eq!(b.kind, SequenceBlockKind::Par);
                        // and 分支作为嵌套块
                        let has_and = b
                            .items
                            .iter()
                            .any(|i| matches!(i, SequenceItem::Block(ib) if ib.kind == SequenceBlockKind::Par));
                        assert!(has_and, "par 的 and 分支应被解析为嵌套块");
                    }
                    _ => panic!("expected par Block"),
                }
            }
            _ => panic!("expected Sequence"),
        }
    }

    #[test]
    fn parses_simple_class_diagram() {
        let input = "classDiagram\nclass Foo\nFoo : int";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Class(class_diag) => {
                assert_eq!(class_diag.classes.len(), 1);
                assert_eq!(class_diag.classes[0].name, "Foo");
            }
            _ => panic!("expected Class"),
        }
    }

    #[test]
    fn parses_class_diagram_with_members() {
        let input = "classDiagram\nclass Animal {\n+name : String\n+age : int\n+makeSound()\n}";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Class(class_diag) => {
                assert_eq!(class_diag.classes.len(), 1);
                assert_eq!(class_diag.classes[0].name, "Animal");
                assert_eq!(class_diag.classes[0].members.len(), 3);
                assert_eq!(
                    class_diag.classes[0].members[0].visibility,
                    Some(Visibility::Public)
                );
                assert!(class_diag.classes[0].members[2].is_method);
            }
            _ => panic!("expected Class"),
        }
    }

    #[test]
    fn parses_class_diagram_with_relations() {
        let input = "classDiagram\nAnimal <|-- Dog\nAnimal <|-- Cat";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Class(class_diag) => {
                assert_eq!(class_diag.relations.len(), 2);
                assert_eq!(class_diag.relations[0].kind, RelationKind::Inheritance);
            }
            _ => panic!("expected Class"),
        }
    }

    #[test]
    fn parses_simple_state_diagram() {
        let input = "stateDiagram\n[*] --> Idle\nIdle --> [*]";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::State(state_diag) => {
                assert_eq!(state_diag.transitions.len(), 2);
            }
            _ => panic!("expected State"),
        }
    }

    #[test]
    fn parses_state_diagram_with_description() {
        let input =
            "stateDiagram\n[*] --> Idle\nIdle --> Processing: start\nProcessing --> [*]: done";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::State(state_diag) => {
                assert_eq!(state_diag.transitions.len(), 3);
                assert_eq!(state_diag.transitions[1].label.as_deref(), Some("start"));
                assert_eq!(state_diag.transitions[2].label.as_deref(), Some("done"));
            }
            _ => panic!("expected State"),
        }
    }

    #[test]
    fn parses_er_diagram() {
        let input = "erDiagram\nCUSTOMER |o--o| ORDER : places\nORDER ||--}| LINEITEM : contains";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Er(er) => {
                assert_eq!(er.relationships.len(), 2);
            }
            _ => panic!("expected ER"),
        }
    }

    #[test]
    fn parses_er_with_entities() {
        let input = "erDiagram\nCUSTOMER {\nint id, string name\n}";
        let result = MermaidParser::parse_mermaid(input);
        assert!(result.is_ok());
    }

    #[test]
    fn detects_no_diagram() {
        let result = MermaidParser::parse_mermaid("just some text");
        assert!(result.is_err());
        match result.unwrap_err() {
            ParseError::Pest(_) => {}
            _ => panic!("expected Pest error"),
        }
    }

    #[test]
    fn handles_empty_input() {
        let result = MermaidParser::parse_mermaid("");
        assert!(result.is_err());
    }

    #[test]
    fn handles_comment_lines() {
        let input = "%% This is a comment\nflowchart TD\n%% Another comment\nA --> B";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(fc) => {
                assert_eq!(fc.edges.len(), 1);
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_flowchart_all_directions() {
        for dir in &["TB", "TD", "BT", "RL", "LR"] {
            let input = format!("flowchart {}\nA --> B", dir);
            let diagram = MermaidParser::parse_mermaid(&input).unwrap();
            match diagram {
                Diagram::Flowchart(fc) => {
                    assert!(fc.direction.is_some());
                }
                _ => panic!("expected Flowchart for direction {}", dir),
            }
        }
    }

    #[test]
    fn parses_flowchart_node_without_shape() {
        let input = "flowchart LR\nA --> B";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(fc) => {
                assert_eq!(fc.nodes[0].shape, None);
                assert_eq!(fc.nodes[0].text, None);
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_sequence_all_arrow_types() {
        for (arrow, expected) in &[
            ("->", MessageArrow::Solid),
            ("->>", MessageArrow::SolidTip),
            ("-->", MessageArrow::Dashed),
            ("-->>", MessageArrow::DashedTip),
            ("-x", MessageArrow::Cross),
            ("-)", MessageArrow::Open),
        ] {
            let input = format!("sequenceDiagram\nA{}B", arrow);
            let diagram = MermaidParser::parse_mermaid(&input).unwrap();
            match diagram {
                Diagram::Sequence(seq) => match &seq.statements[0] {
                    SequenceStatement::Message(m) => {
                        assert_eq!(m.arrow, *expected, "failed for arrow {}", arrow)
                    }
                    _ => panic!("expected Message"),
                },
                _ => panic!("expected Sequence"),
            }
        }
    }

    #[test]
    fn parses_state_diagram_v2() {
        let input = "stateDiagram-v2\n[*] --> Idle";
        let diagram = MermaidParser::parse_mermaid(input).unwrap();
        match diagram {
            Diagram::State(_) => {}
            _ => panic!("expected State"),
        }
    }

    #[test]
    fn parses_class_multiple_relations() {
        let input = "classDiagram\nclassA --> classB\nclassC ..> classD";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Class(cd) => {
                assert_eq!(cd.relations.len(), 2);
                assert_eq!(cd.relations[0].kind, RelationKind::Association);
                assert_eq!(cd.relations[1].kind, RelationKind::Dependency);
            }
            _ => panic!("expected Class"),
        }
    }

    #[test]
    fn parses_er_cardinality() {
        let input = "erDiagram\nA |o--}| B : has";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Er(er) => {
                assert_eq!(er.relationships.len(), 1);
                assert_eq!(
                    er.relationships[0].cardinality_first,
                    Cardinality::ZeroOrOne
                );
                assert_eq!(
                    er.relationships[0].cardinality_second,
                    Cardinality::OneOrMany
                );
            }
            _ => panic!("expected ER"),
        }
    }

    #[test]
    fn serde_roundtrip() {
        let input = "flowchart LR\nA --> B";
        let diagram = MermaidParser::parse_mermaid(input).unwrap();
        let json = serde_json::to_string(&diagram).unwrap();
        let deserialized: Diagram = serde_json::from_str(&json).unwrap();
        assert_eq!(diagram, deserialized);
    }

    #[test]
    fn parses_simple_pie() {
        let input = "pie\n\"A\" : 30\n\"B\" : 70";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Pie(pie) => {
                assert_eq!(pie.data.len(), 2);
                assert_eq!(pie.data[0].label, "\"A\"");
                assert_eq!(pie.data[0].value, "30");
                assert_eq!(pie.data[1].label, "\"B\"");
                assert_eq!(pie.data[1].value, "70");
            }
            _ => panic!("expected Pie"),
        }
    }

    #[test]
    fn parses_pie_with_title() {
        let input = "pie\ntitle My Title\n\"A\" : 30\n\"B\" : 70";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Pie(pie) => {
                assert_eq!(pie.title, Some("My Title".to_string()));
                assert_eq!(pie.data.len(), 2);
            }
            _ => panic!("expected Pie"),
        }
    }

    #[test]
    fn parses_pie_with_showdata() {
        let input = "pie\nshowData\n\"A\" : 30\n\"B\" : 70";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Pie(pie) => {
                assert!(pie.show_data);
                assert_eq!(pie.data.len(), 2);
            }
            _ => panic!("expected Pie"),
        }
    }

    #[test]
    fn parses_simple_timeline() {
        let input = "timeline\ntitle History\nsection Age\nEvent 1 : 1900";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Timeline(tl) => {
                assert_eq!(tl.title, Some("History".to_string()));
                assert_eq!(tl.sections.len(), 1);
                assert_eq!(tl.sections[0].name, "Age");
                assert_eq!(tl.sections[0].events.len(), 1);
                assert_eq!(tl.sections[0].events[0], "1900");
            }
            _ => panic!("expected Timeline"),
        }
    }

    #[test]
    fn parses_timeline_multiple_events() {
        let input = "timeline\nsection Era\nAlpha : 100\nBeta : 200";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Timeline(tl) => {
                assert_eq!(tl.sections.len(), 1);
                assert_eq!(tl.sections[0].events.len(), 2);
                assert_eq!(tl.sections[0].events[0], "100");
                assert_eq!(tl.sections[0].events[1], "200");
            }
            _ => panic!("expected Timeline"),
        }
    }

    #[test]
    fn parses_timeline_title_case_insensitive() {
        let input = "timeline\nTitle My History\nsection Age\nEvent 1 : 1900";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Timeline(tl) => {
                assert_eq!(tl.title, Some("My History".to_string()));
            }
            _ => panic!("expected Timeline"),
        }
    }

    #[test]
    fn parses_simple_gitgraph() {
        let input = "gitGraph\ncommit\nbranch feature\ncheckout feature\ncommit\ncheckout main\nmerge feature";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::GitGraph(gg) => {
                assert_eq!(gg.statements.len(), 6);
                assert_eq!(gg.statements[0], GitGraphStatement::Commit { tag: None });
                assert_eq!(
                    gg.statements[1],
                    GitGraphStatement::Branch {
                        name: "feature".to_string()
                    }
                );
                assert_eq!(
                    gg.statements[2],
                    GitGraphStatement::Checkout {
                        branch: "feature".to_string()
                    }
                );
                assert_eq!(gg.statements[3], GitGraphStatement::Commit { tag: None });
                assert_eq!(
                    gg.statements[4],
                    GitGraphStatement::Checkout {
                        branch: "main".to_string()
                    }
                );
                assert_eq!(
                    gg.statements[5],
                    GitGraphStatement::Merge {
                        branch: "feature".to_string()
                    }
                );
            }
            _ => panic!("expected GitGraph"),
        }
    }

    #[test]
    fn parses_gitgraph_with_tag() {
        let input = "gitGraph\ncommit tag: \"v1.0\"\ncommit";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::GitGraph(gg) => {
                assert_eq!(gg.statements.len(), 2);
                assert_eq!(
                    gg.statements[0],
                    GitGraphStatement::Commit {
                        tag: Some("\"v1.0\"".to_string())
                    }
                );
                assert_eq!(gg.statements[1], GitGraphStatement::Commit { tag: None });
            }
            _ => panic!("expected GitGraph"),
        }
    }

    #[test]
    fn parses_subgraph_title_containing_end_keyword() {
        // subgraph 标题含 "end" 子串（如 backend）不应被误判为结束标志
        let input = "flowchart TD\nsubgraph backend services\nA --> B\nend";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(fc) => {
                assert_eq!(fc.subgraphs.len(), 1);
                assert_eq!(fc.subgraphs[0].title.as_deref(), Some("backend services"));
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn parses_subgraph_title_ends_with_end_keyword() {
        // 标题恰好以 end 结尾（如 frontend）也应正常，真正的结束关键字是独立的 end 行
        let input = "flowchart TD\nsubgraph frontend\nA --> B\nend";
        match MermaidParser::parse_mermaid(input).unwrap() {
            Diagram::Flowchart(fc) => {
                assert_eq!(fc.subgraphs[0].title.as_deref(), Some("frontend"));
            }
            _ => panic!("expected Flowchart"),
        }
    }

    #[test]
    fn note_left_right_require_of() {
        // left/right 必须跟 of
        assert!(MermaidParser::parse_mermaid("sequenceDiagram\nnote left of alice: hi").is_ok());
        assert!(MermaidParser::parse_mermaid("sequenceDiagram\nnote right of bob: hi").is_ok());
        // left 缺 of 应失败
        assert!(MermaidParser::parse_mermaid("sequenceDiagram\nnote left alice: hi").is_err());
    }

    #[test]
    fn note_over_rejects_of() {
        // over 后不能跟 of
        assert!(MermaidParser::parse_mermaid("sequenceDiagram\nnote over alice,bob: hi").is_ok());
        assert!(MermaidParser::parse_mermaid("sequenceDiagram\nnote over of alice: hi").is_err());
    }

    #[test]
    fn sequence_statements_require_newline() {
        // 同行两个语句应被拒绝（语句间必须换行）
        assert!(
            MermaidParser::parse_mermaid("sequenceDiagram\nparticipant A participant B").is_err()
        );
        assert!(
            MermaidParser::parse_mermaid("sequenceDiagram\nparticipant A\nparticipant B").is_ok()
        );
    }
}
