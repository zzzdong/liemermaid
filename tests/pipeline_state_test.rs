//! state 图走新四阶段管线（extract → measure → layout → materialize → paint）的验证。
//!
//! 验证：
//! - extract 产出的 Unigraph 节点/边数量与形状正确；
//! - `[*]` 映射为 `__start__`/`__end__`（StartDot/EndDot）；
//! - fork/join 映射为 Bar；
//! - 转移边带 label；
//! - 端到端渲染不 panic，产出含 start/end/状态节点。

use liemermaid::MermaidParser;
use liemermaid::builder::extract;
use liemermaid::builder::ir::shape::ShapeKind;
use liemermaid::builder::ir::unigraph::EdgeKind;
use liemermaid::builder::layout::engine;
use liemermaid::builder::materialize;
use liemermaid::builder::measure;
use liemermaid::builder::paint;

fn parse(src: &str) -> liemermaid::ast::Diagram {
    MermaidParser::parse_mermaid(src).expect("parse failed")
}

#[test]
fn state_basic_shapes_and_transitions() {
    let src = "stateDiagram-v2\n\
        [*] --> Idle\n\
        Idle --> Running\n\
        Running --> Done\n\
        Done --> [*]";
    let diagram = parse(src);
    let ug = extract::run(&diagram).expect("extract failed");
    // 节点：__start__ / Idle / Running / Done / __end__
    assert_eq!(ug.nodes.len(), 5, "应有 5 个节点");
    let start = ug
        .nodes
        .iter()
        .find(|n| n.id == "__start__")
        .expect("start 节点");
    assert_eq!(start.shape, ShapeKind::StartDot, "start 应为 StartDot");
    let end = ug
        .nodes
        .iter()
        .find(|n| n.id == "__end__")
        .expect("end 节点");
    assert_eq!(end.shape, ShapeKind::EndDot, "end 应为 EndDot");
    let idle = ug.nodes.iter().find(|n| n.id == "Idle").expect("Idle 节点");
    assert_eq!(idle.shape, ShapeKind::Rounded, "普通状态应为圆角矩形");
    assert_eq!(ug.edges.len(), 4, "应有 4 条转移边");
    assert!(
        ug.edges.iter().all(|e| e.kind == EdgeKind::StateTransition),
        "转移边 kind 应为 StateTransition"
    );
}

#[test]
fn state_fork_join_bar_nodes() {
    let src = "stateDiagram-v2\n\
        state fork_state <<fork>>\n\
        [*] --> fork_state\n\
        fork_state --> State2\n\
        fork_state --> State3\n\
        State2 --> join_state\n\
        State3 --> join_state\n\
        state join_state <<join>>\n\
        join_state --> State4\n\
        State4 --> [*]";
    let diagram = parse(src);
    let ug = extract::run(&diagram).expect("extract failed");
    let fork = ug
        .nodes
        .iter()
        .find(|n| n.id == "fork_state")
        .expect("fork 节点");
    assert_eq!(fork.shape, ShapeKind::Bar, "fork 应为 Bar");
    // 官方行为：join_state 先被转移引用创建为普通状态，后续 `<<join>>` 声明不再生效
    // （golden：join_state 是带 label 的普通圆角框，不是横条）。
    let join = ug
        .nodes
        .iter()
        .find(|n| n.id == "join_state")
        .expect("join 节点");
    assert_eq!(
        join.shape,
        ShapeKind::Rounded,
        "join_state 应为普通状态框（官方行为）"
    );
}

#[test]
fn state_edge_labels_present() {
    let src = "stateDiagram-v2\n\
        state \"流量处理\" as s1\n\
        s2 : 等待输入\n\
        s3\n\
        s1 --> s2\n\
        s2 --> s3 : done\n\
        s3 --> s1 : retry";
    let diagram = parse(src);
    let ug = extract::run(&diagram).expect("extract failed");
    // s2 --> s3 和 s3 --> s1 有 label
    let labeled = ug.edges.iter().filter(|e| e.label_text.is_some()).count();
    assert_eq!(labeled, 2, "应有 2 条带 label 的转移边");
    // s1 的描述文本应为 "流量处理"
    let s1 = ug.nodes.iter().find(|n| n.id == "s1").expect("s1");
    assert!(!s1.label.is_measured());
    // measure 后 label 文本存在
    let ug = measure::measure_all(ug);
    let s1 = ug.nodes.iter().find(|n| n.id == "s1").expect("s1");
    let m = s1.label.as_measured().expect("应已测量");
    assert_eq!(m.text, "流量处理");
}

#[test]
fn state_end_to_end_renders_without_panic() {
    let src = "stateDiagram-v2\n\
        [*] --> Idle\n\
        Idle --> Running\n\
        Running --> Done\n\
        Done --> [*]";
    let diagram = parse(src);
    let ug = extract::run(&diagram).expect("extract failed");
    let ug = measure::measure_all(ug);
    let (gg, style) = engine::run(&ug).expect("layout failed");
    let sg = materialize::run(&gg, &style);
    let scene = paint::run(&sg);
    assert!(!scene.nodes.is_empty(), "state 渲染应产出图元");
    // 5 节点（每个节点有形状；EndDot/DoubleCircle 双环产生 2 个 shape） + 4 边
    let shape_count = sg
        .items
        .iter()
        .filter(|i| matches!(i, liemermaid::builder::ir::SceneItem::Shape { .. }))
        .count();
    // start(1) + Idle/Running/Done(各1) + end 双环(2) + 4 边标签占位(无标签则无) = 6
    assert!(shape_count >= 5, "至少 5 个形状，got {shape_count}");
}

/// 复合状态（`state X { ... }`）对齐官方 golden `state__composite`：
/// - 框内 `[*]` 生成为真实起止圆点节点（`__start__X` / `__end__X`）；
/// - 框内边（圆点→Inner1、Inner1→Inner2、Inner2→圆点）全部保留，
///   加上外层 3 条边共 6 条转移（官方 golden 即 6 条）；
/// - 容器为 StateComposite，含标题。
#[test]
fn state_composite_inner_stars_become_dots_and_edges() {
    let src = "stateDiagram-v2\n\
        [*] --> Outer\n\
        state Outer {\n\
            [*] --> Inner1\n\
            Inner1 --> Inner2\n\
            Inner2 --> [*]\n\
        }\n\
        Outer --> Final\n\
        Final --> [*]";
    let diagram = parse(src);
    let ug = extract::run(&diagram).expect("extract failed");

    // 框内起止圆点存在。
    let start_dot = ug.nodes.iter().find(|n| n.id == "__start__Outer");
    let end_dot = ug.nodes.iter().find(|n| n.id == "__end__Outer");
    assert!(start_dot.is_some(), "复合状态内应有起始圆点 __start__Outer");
    assert!(end_dot.is_some(), "复合状态内应有终止圆点 __end__Outer");
    assert_eq!(start_dot.unwrap().shape, ShapeKind::StartDot);
    assert_eq!(end_dot.unwrap().shape, ShapeKind::EndDot);

    // 6 条转移边（官方 golden state__composite 为 6 条）。
    let transitions: Vec<String> = ug
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::StateTransition)
        .map(|e| format!("{}->{}", e.source, e.target))
        .collect();
    assert_eq!(transitions.len(), 6, "应有 6 条转移边: {transitions:?}");
    let has = |s: &str, t: &str| transitions.iter().any(|e| e == &format!("{s}->{t}"));
    assert!(has("__start__", "__start__Outer"), "外层 [*] → 框内圆点");
    assert!(has("__start__Outer", "Inner1"), "框内圆点 → Inner1");
    assert!(has("Inner1", "Inner2"), "Inner1 → Inner2");
    assert!(has("Inner2", "__end__Outer"), "Inner2 → 框内圆点");
    assert!(has("__end__Outer", "Final"), "框内圆点 → Final");
    assert!(has("Final", "__end__"), "Final → [*]");

    // 容器：StateComposite + 标题 + 4 个成员（两圆点 + Inner1/Inner2）。
    let sg = ug
        .subgraphs
        .iter()
        .find(|s| s.id == "Outer")
        .expect("Outer 容器");
    assert_eq!(sg.member_ids.len(), 4, "容器成员应含两圆点与两个内部状态");
    assert_eq!(sg.title.as_deref(), Some("Outer"));

    // 端到端渲染不 panic。
    let ug = measure::measure_all(ug);
    let (gg, style) = engine::run(&ug).expect("layout failed");
    let sg = materialize::run(&gg, &style);
    assert!(!paint::run(&sg).nodes.is_empty(), "复合状态渲染应产出图元");
}
