//! 语法覆盖与回归测试。
//!
//! 目标：**常见 mermaid 写法必须产出内容**（不得静默丢语句），且渲染过程
//! 不 panic、不产出 NaN / 非正尺寸画布。
//!
//! 本文件是多个真实缺陷的回归网：
//! - `identifier` 曾吞掉 `-`，导致 `A-->B` 整条边被丢弃（空白画布）
//! - winnow 解析失败不回滚输入，兜底 `skip_line` 把后续合法语句整行吃掉
//! - 消息语句内部用了会跨行的空白，把 `autonumber` 的下一行消息并进上一句
//! - 画布尺寸小于 2×边距时 `scale` 变负，产出 `viewBox="0 0 0 -20.41"`

use liemermaid::ast::{ArrowType, Diagram, RelationKind, SequenceBlockKind};
use liemermaid::{MermaidParser, render};

/// 去掉 `<svg ...>` 根节点后的内容长度（0 表示什么都没画）。
fn body_len(svg: &str) -> usize {
    let s = svg.split_once('>').map(|(_, r)| r).unwrap_or("");
    s.replace("</svg>", "").trim().len()
}

fn parse(src: &str) -> Diagram {
    MermaidParser::parse_mermaid(src).expect("解析应成功")
}

fn flowchart_of(src: &str) -> liemermaid::ast::Flowchart {
    match parse(src) {
        Diagram::Flowchart(fc) => fc,
        other => panic!("期望 Flowchart，得到 {other:?}"),
    }
}

fn render_ok(src: &str) -> String {
    let svg = render(src, 900, 700).expect("渲染应成功");
    assert!(
        !svg.contains("NaN") && !svg.contains("Infinity"),
        "SVG 不应含 NaN/Infinity: {src:?}"
    );
    svg
}

fn assert_non_empty(src: &str) {
    let svg = render_ok(src);
    assert!(
        body_len(&svg) > 0,
        "应产出内容，实际为空画布: {src:?}\n{svg}"
    );
}

// ============================================================
// Flowchart：箭头符号（无空格写法回归）
// ============================================================

/// `identifier` 曾把 `-` 计入标识符，`A-->B` 被切成 `A--` + `>B` 而整行丢弃。
#[test]
fn spaceless_arrow_forms_parse() {
    let fc = flowchart_of("flowchart TD\n A-->B");
    assert_eq!(fc.edges.len(), 1, "`A-->B` 应解析出 1 条边");
    assert_eq!(fc.edges[0].source, "A");
    assert_eq!(fc.edges[0].target, "B");
    assert_eq!(fc.edges[0].arrow_type, ArrowType::Solid);
    assert_eq!(fc.nodes.len(), 2, "边端点应自动补为节点");
}

#[test]
fn all_spaceless_arrow_types() {
    let fc = flowchart_of("flowchart TD\n A---B\n C-.->D\n E==>F\n G--oH\n I--xJ\n K<-->L");
    let kinds: Vec<_> = fc.edges.iter().map(|e| e.arrow_type.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            ArrowType::NoArrow,
            ArrowType::Dotted,
            ArrowType::Thick,
            ArrowType::Circle,
            ArrowType::Cross,
            ArrowType::Both,
        ]
    );
}

#[test]
fn spaceless_labeled_edges() {
    let fc = flowchart_of("flowchart TD\n A-->|yes|B\n C--|no|-->D\n E--maybe-->F");
    assert_eq!(fc.edges.len(), 3);
    assert_eq!(fc.edges[0].label.as_deref(), Some("yes"));
    assert_eq!(fc.edges[1].label.as_deref(), Some("no"));
    assert_eq!(fc.edges[2].label.as_deref(), Some("maybe"));
}

// ============================================================
// Flowchart：链式链接与端点形状
// ============================================================

/// `A --> B --> C` 曾只解析出第一条边，后续段被兜底逻辑丢弃。
#[test]
fn chained_edges_are_all_parsed() {
    let fc = flowchart_of("flowchart TD\n A --> B --> C --> D");
    assert_eq!(fc.edges.len(), 3, "3 段链应有 3 条边");
    assert_eq!(fc.nodes.len(), 4);
    assert_eq!(
        fc.edges
            .iter()
            .map(|e| format!("{}->{}", e.source, e.target))
            .collect::<Vec<_>>(),
        vec!["A->B", "B->C", "C->D"]
    );
}

/// 端点自带形状（mermaid 最常见写法）必须同时登记节点，否则标签丢失。
#[test]
fn edge_endpoints_may_declare_shapes() {
    let fc = flowchart_of("flowchart TD\n A[Start] --> B{Dec} --> C[End]");
    assert_eq!(fc.edges.len(), 2);
    assert_eq!(fc.nodes.len(), 3);
    let texts: Vec<_> = fc
        .nodes
        .iter()
        .map(|n| n.text.clone().unwrap_or_default())
        .collect();
    assert_eq!(texts, vec!["Start", "Dec", "End"]);
}

#[test]
fn amp_multi_endpoints_expand_to_multiple_edges() {
    let fc = flowchart_of("flowchart TD\n A & B --> C");
    let pairs: Vec<_> = fc
        .edges
        .iter()
        .map(|e| format!("{}->{}", e.source, e.target))
        .collect();
    assert_eq!(pairs, vec!["A->C", "B->C"]);

    let fc = flowchart_of("flowchart TD\n A --> B & C");
    let pairs: Vec<_> = fc
        .edges
        .iter()
        .map(|e| format!("{}->{}", e.source, e.target))
        .collect();
    assert_eq!(pairs, vec!["A->B", "A->C"]);
}

#[test]
fn node_declaration_lines_still_parse() {
    let fc = flowchart_of("flowchart TD\n A[Start]\n B[End]\n A --> B");
    assert_eq!(fc.nodes.len(), 2);
    assert_eq!(fc.edges.len(), 1);
}

// ============================================================
// Sequence：指令行不得吞掉后续消息
// ============================================================

/// `autonumber` 后的消息曾因语句内空白跨行而被并入指令行，凭空造出参与者。
#[test]
fn autonumber_does_not_swallow_next_message() {
    let d = match parse("sequenceDiagram\n autonumber\n A->>B: hi") {
        Diagram::Sequence(s) => s,
        other => panic!("期望 Sequence，得到 {other:?}"),
    };
    assert_eq!(d.statements.len(), 1, "应解析出 1 条消息");
    match &d.statements[0] {
        liemermaid::ast::SequenceStatement::Message(m) => {
            assert_eq!(m.from, "A");
            assert_eq!(m.to, "B");
        }
        other => panic!("期望 Message，得到 {other:?}"),
    }
}

#[test]
fn sequence_block_kinds_parse() {
    for (kw, kind) in [
        ("loop", SequenceBlockKind::Loop),
        ("alt", SequenceBlockKind::Alt),
        ("opt", SequenceBlockKind::Opt),
        ("par", SequenceBlockKind::Par),
        ("critical", SequenceBlockKind::Critical),
        ("break", SequenceBlockKind::Break),
        ("rect", SequenceBlockKind::Rect),
    ] {
        let src = format!("sequenceDiagram\n {kw} label\n A->>B: x\n end");
        let d = match parse(&src) {
            Diagram::Sequence(s) => s,
            other => panic!("期望 Sequence，得到 {other:?}"),
        };
        assert_eq!(d.statements.len(), 1, "{kw} 块应解析为 1 条语句");
        match &d.statements[0] {
            liemermaid::ast::SequenceStatement::Block(b) => assert_eq!(b.kind, kind),
            other => panic!("{kw} 应解析为 Block，得到 {other:?}"),
        }
    }
}

#[test]
fn sequence_open_arrows_parse() {
    let d = match parse("sequenceDiagram\n A-)B: 1\n C--)D: 2") {
        Diagram::Sequence(s) => s,
        other => panic!("期望 Sequence，得到 {other:?}"),
    };
    assert_eq!(d.statements.len(), 2, "`-)` / `--)` 是合法 mermaid 箭头");
}

/// alt / par / critical 的多分支：`else` / `and` / `option` 开启新分支段。
#[test]
fn sequence_alt_par_branches_parse() {
    let d = match parse(
        "sequenceDiagram\n A->>B: start\n alt first\n A->>B: ok\n else second\n B-->>A: err\n else third\n A->>B: retry\n end",
    ) {
        Diagram::Sequence(s) => s,
        other => panic!("期望 Sequence，得到 {other:?}"),
    };
    // 首条消息 + alt 块。
    assert_eq!(d.statements.len(), 2, "alt 块应解析为 1 条语句");
    match &d.statements[1] {
        liemermaid::ast::SequenceStatement::Block(b) => {
            assert_eq!(b.kind, SequenceBlockKind::Alt);
            assert_eq!(b.branches.len(), 3, "else × 2 应拆出 3 个分支段");
            assert_eq!(b.branches[0].label, None, "首分支无独立条件标签");
            assert_eq!(b.branches[1].label.as_deref(), Some("second"));
            assert_eq!(b.branches[2].label.as_deref(), Some("third"));
            // 首分支含 1 条消息，后续分支各含 1 条。
            assert_eq!(b.branches[0].items.len(), 1);
            assert_eq!(b.branches[1].items.len(), 1);
            assert_eq!(b.branches[2].items.len(), 1);
        }
        other => panic!("期望 Block，得到 {other:?}"),
    }
}

/// par 用 `and` 分隔、critical 用 `option` 分隔，各自拆出多分支。
#[test]
fn sequence_par_critical_branch_separators() {
    for (kw, sep, expected_kind) in [
        ("par", "and", SequenceBlockKind::Par),
        ("critical", "option", SequenceBlockKind::Critical),
    ] {
        let src =
            format!("sequenceDiagram\n {kw} header\n A->>B: p1\n {sep} p2\n A->>B: p2msg\n end");
        let d = match parse(&src) {
            Diagram::Sequence(s) => s,
            other => panic!("期望 Sequence，得到 {other:?}"),
        };
        match &d.statements[0] {
            liemermaid::ast::SequenceStatement::Block(b) => {
                assert_eq!(b.kind, expected_kind, "{kw} 块类型");
                assert_eq!(b.branches.len(), 2, "{kw} 应拆出 2 个分支段");
                assert_eq!(b.branches[0].label, None);
                assert_eq!(b.branches[1].label.as_deref(), Some("p2"), "{kw} 分支条件");
                assert_eq!(b.branches[0].items.len(), 1);
                assert_eq!(b.branches[1].items.len(), 1);
            }
            other => panic!("期望 Block，得到 {other:?}"),
        }
    }
}

/// 单分支块（loop / opt / break / rect）内的 `else` 普通行不被误判为分隔。
#[test]
fn single_branch_blocks_ignore_branch_separator() {
    let d = match parse("sequenceDiagram\n loop L\n A->>B: x\n else not-a-branch\n A->>B: y\n end")
    {
        Diagram::Sequence(s) => s,
        other => panic!("期望 Sequence，得到 {other:?}"),
    };
    match &d.statements[0] {
        liemermaid::ast::SequenceStatement::Block(b) => {
            assert_eq!(b.kind, SequenceBlockKind::Loop);
            assert_eq!(b.branches.len(), 1, "loop 内 else 不应开启新分支");
            assert_eq!(
                b.branches[0].items.len(),
                2,
                "else 行被安全跳过，消息不丢失"
            );
        }
        other => panic!("期望 Block，得到 {other:?}"),
    }
}

/// 无标签块不把下一行语句吞成标签（此前 `loop\n msg` 会把消息行并进 label）。
#[test]
fn unlabeled_block_does_not_swallow_next_line() {
    let d = match parse("sequenceDiagram\n loop\n A->>B: x\n end\n A->>C: y") {
        Diagram::Sequence(s) => s,
        other => panic!("期望 Sequence，得到 {other:?}"),
    };
    assert_eq!(d.statements.len(), 2, "块外消息不应被吞掉");
    match &d.statements[0] {
        liemermaid::ast::SequenceStatement::Block(b) => {
            assert_eq!(b.kind, SequenceBlockKind::Loop);
            assert_eq!(b.label, None, "无标签块 label 应为空");
            assert_eq!(b.branches[0].items.len(), 1);
        }
        other => panic!("期望 Block，得到 {other:?}"),
    }
}

/// 以 `end` 开头的参与者名不应被误判为块结束。
#[test]
fn participant_named_end_is_not_block_end() {
    let d = match parse("sequenceDiagram\n loop L\n endpoint->>A: x\n end\n A->>B: y") {
        Diagram::Sequence(s) => s,
        other => panic!("期望 Sequence，得到 {other:?}"),
    };
    assert_eq!(d.statements.len(), 2, "块外的消息不应被吞掉");
}

// ============================================================
// Class：关系符号
// ============================================================

#[test]
fn class_relation_kinds_parse() {
    let d = match parse(
        "classDiagram\n A <|-- B\n C ..|> D\n E ..> F\n G -- H\n I .. J\n K *-- L\n M o-- N",
    ) {
        Diagram::Class(c) => c,
        other => panic!("期望 Class，得到 {other:?}"),
    };
    let kinds: Vec<_> = d.relations.iter().map(|r| r.kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            RelationKind::Inheritance,
            RelationKind::Realization,
            RelationKind::Dependency,
            RelationKind::Link,
            RelationKind::Dashed,
            RelationKind::Composition,
            RelationKind::Aggregation,
        ]
    );
}

// ============================================================
// ER：`as` 别名
// ============================================================

#[test]
fn er_entity_alias_is_used_as_display_name() {
    let d = match parse("erDiagram\n CUSTOMER as C ||--o{ ORDER as O : places") {
        Diagram::Er(e) => e,
        other => panic!("期望 Er，得到 {other:?}"),
    };
    assert_eq!(d.relationships.len(), 1);
    assert_eq!(d.relationships[0].first_entity, "C");
    assert_eq!(d.relationships[0].second_entity, "O");
}

// ============================================================
// Unicode 标识符
// ============================================================

#[test]
fn unicode_identifiers_parse() {
    let fc = flowchart_of("flowchart TD\n 开始[开始] --> 结束[结束]");
    assert_eq!(fc.edges.len(), 1);
    let ids: Vec<_> = fc.nodes.iter().map(|n| n.id.clone()).collect();
    assert!(
        ids.contains(&"开始".to_string()) && ids.contains(&"结束".to_string()),
        "中文标识符应被识别，实际 {ids:?}"
    );
}

/// sequence 的参与者名不得吞掉 `-x` 箭头（故用不含连字符的严格标识符）。
#[test]
fn sequence_cross_arrow_keeps_participant_names_intact() {
    let d = match parse("sequenceDiagram\n A-xB: 1") {
        Diagram::Sequence(s) => s,
        other => panic!("期望 Sequence，得到 {other:?}"),
    };
    assert_eq!(d.statements.len(), 1);
    match &d.statements[0] {
        liemermaid::ast::SequenceStatement::Message(m) => {
            assert_eq!(m.from, "A");
            assert_eq!(m.to, "B");
        }
        other => panic!("期望 Message，得到 {other:?}"),
    }
}

// ============================================================
// 渲染冒烟：常见写法都必须有内容且不 panic
// ============================================================

#[test]
fn common_syntax_renders_content() {
    let cases: Vec<&str> = vec![
        "flowchart TD\n A-->B",
        "flowchart TD\n A --> B --> C",
        "flowchart TD\n A[Start] --> B{Dec} --> C[End]",
        "flowchart TD\n A & B --> C",
        "flowchart TD\n A --> B & C",
        "flowchart TD\n A-->|yes|B",
        "flowchart TD\n subgraph S\n A-->B\n end\n B-->C",
        "flowchart TD\n A[A]-->A",
        "flowchart LR\n A-->B-->C-->D-->E-->F",
        "flowchart TD\n A-->B\n style A fill:#f9f\n linkStyle 0 stroke:#ff3",
        "flowchart TD\n A[中文] --> B[English]",
        "sequenceDiagram\n autonumber\n A->>B: hi",
        "sequenceDiagram\n participant A\n participant B\n A->>+B: 1\n B-->>-A: 2",
        "sequenceDiagram\n critical c\n A->>B: 1\n end",
        "sequenceDiagram\n Note over A,B: n\n A->>B: x",
        "classDiagram\n class Animal\n Animal : +int age\n Animal : +eat()",
        "classDiagram\n Duck ..|> Flyable",
        "erDiagram\n CUSTOMER ||--o{ ORDER : places\n CUSTOMER { string name }",
        "erDiagram\n CUSTOMER as C ||--o{ ORDER as O : places",
        "pie showData\n \"A\": 30\n \"B\": 70",
        "gitGraph\n commit\n branch dev\n commit\n checkout main\n merge dev",
        "timeline\n title H\n section S\n 2000 : e1\n 2001 : e2",
        "stateDiagram-v2\n [*] --> A\n A --> B\n B --> [*]",
        "stateDiagram-v2\n state A {\n [*] --> A1\n A1 --> [*]\n }\n [*] --> A",
    ];
    for src in &cases {
        assert_non_empty(src);
    }
}

// ============================================================
// 画布尺寸：退化输入不得产出负尺寸 viewBox
// ============================================================

fn viewbox(svg: &str) -> [f64; 4] {
    let vb = svg
        .split_once("viewBox=\"")
        .and_then(|(_, s)| s.split_once('"'))
        .expect("应存在 viewBox")
        .0
        .to_string();
    let nums: Vec<f64> = vb
        .split_whitespace()
        .map(|n| n.parse().expect("viewBox 应为数字"))
        .collect();
    assert_eq!(nums.len(), 4, "viewBox 应有 4 个数: {vb}");
    [nums[0], nums[1], nums[2], nums[3]]
}

/// 配置尺寸小于两倍边距时，`scale` 曾变负并产出负尺寸画布。
#[test]
fn degenerate_canvas_sizes_stay_positive() {
    for (w, h) in [(0u32, 0u32), (1, 1), (4, 4), (16, 16), (u32::MAX, u32::MAX)] {
        let svg = render_ok_maybe_empty("flowchart TD\n A --> B", w, h);
        let [_, _, vw, vh] = viewbox(&svg);
        assert!(vw > 0.0, "{w}x{h} 画布宽应为正，实际 {vw}");
        assert!(vh > 0.0, "{w}x{h} 画布高应为正，实际 {vh}");
        assert!(vw.is_finite() && vh.is_finite(), "{w}x{h} 画布尺寸应有限");
    }
}

fn render_ok_maybe_empty(src: &str, w: u32, h: u32) -> String {
    let svg = render(src, w, h).expect("渲染应成功");
    assert!(!svg.contains("NaN"), "SVG 不应含 NaN");
    svg
}

/// 内容超出配置上限时只缩小、不裁切，且不放大。
#[test]
fn oversized_content_is_scaled_not_upscaled() {
    let small = render_ok("flowchart TD\n A --> B");
    let [_, _, w1, h1] = viewbox(&small);

    let big = render("flowchart LR\n A-->B-->C-->D-->E-->F-->G-->H", 240, 240).expect("渲染应成功");
    let [_, _, w2, h2] = viewbox(&big);
    assert!(w2 <= 240.0 + 1.0, "超宽内容应被缩小: {w2}");
    assert!(h2 <= 240.0 + 1.0, "超高内容应被缩小: {h2}");

    // 小内容不应被放大到 240（画布贴合内容）。
    let fitted = render("flowchart TD\n A --> B", 240, 240).expect("渲染应成功");
    let [_, _, w3, h3] = viewbox(&fitted);
    assert!(
        (w3 - w1).abs() < 0.5,
        "内容已装得下时不应缩放: {w3} vs {w1}"
    );
    assert!(
        (h3 - h1).abs() < 0.5,
        "内容已装得下时不应缩放: {h3} vs {h1}"
    );
}

// ============================================================
// 健壮性：病态输入不得 panic
// ============================================================

#[test]
fn pathological_inputs_do_not_panic() {
    let inputs: Vec<&str> = vec![
        "",
        "   \n\t \n",
        "not a diagram",
        "flowchart",
        "flowchart TD",
        "flowchart TD\n A",
        "flowchart TD\n A[",
        "flowchart TD\n -->",
        "flowchart TD\n subgraph\n",
        "flowchart TD\n A ==> ",
        "pie",
        "pie\n",
        "sequenceDiagram",
        "sequenceDiagram\n ->",
        "classDiagram",
        "erDiagram",
        "gitGraph",
        "timeline",
        "stateDiagram-v2",
        "flowchart TD\n A[\"unterminated --> B",
    ];
    for src in &inputs {
        // 解析/渲染可以失败（返回 Err），但绝不能 panic。
        let _ = render(src, 800, 600);
        let _ = liemermaid::render_png(src, 200, 200);
    }
}
