/// 辅助函数：解析 Mermaid 文本并通过 lievisual 后端渲染为 SVG 字符串
fn render_to_svg(mermaid_text: &str, width: u32, height: u32) -> String {
    liemermaid::render(mermaid_text, width, height).expect("SVG rendering should succeed")
}

/// 剥掉 XML 标签，提取纯文本内容
fn strip_xml(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut in_tag = false;
    for ch in svg.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

// ============================================================
// Pie 图表测试
// ============================================================

#[test]
fn pie_diagram_produces_valid_svg_structure() {
    let svg = render_to_svg(
        "pie\ntitle My Pie\n\"A\": 30\n\"B\": 50\n\"C\": 20",
        600,
        400,
    );
    let text = strip_xml(&svg);

    // 1. SVG 文档结构
    assert!(
        svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg""#),
        "SVG should start with proper namespace"
    );
    assert!(svg.ends_with("</svg>\n"), "SVG should end with closing tag");
    assert!(svg.contains("width=\"600.00\""), "SVG width should be 600");
    assert!(
        svg.contains("height=\"400.00\""),
        "SVG height should be 400"
    );

    // 2. 饼图应有 3 个 <path> 元素（3个扇区）
    let path_count = svg.matches("<path ").count();
    assert_eq!(
        path_count, 3,
        "Pie with 3 data entries should have 3 path elements"
    );

    // 3. 标题
    assert!(text.contains("My Pie"), "SVG should contain the pie title");

    // 4. 数据标签（去除引号后）
    assert!(text.contains("A"), "SVG should contain data label A");
    assert!(text.contains("B"), "SVG should contain data label B");
    assert!(text.contains("C"), "SVG should contain data label C");

    // 5. 百分比显示（30/100=30%, 50/100=50%, 20/100=20%）
    assert!(text.contains("30.0%"), "SVG should show 30% for A");
    assert!(text.contains("50.0%"), "SVG should show 50% for B");
    assert!(text.contains("20.0%"), "SVG should show 20% for C");
}

#[test]
fn pie_diagram_show_data_displays_values() {
    // 注意：showData 需要单独一行（语法要求）
    let svg = render_to_svg(
        "pie\nshowData\ntitle Sales\n\"X\": 100\n\"Y\": 200",
        600,
        400,
    );
    let text = strip_xml(&svg);

    // showData 开启时，应显示具体数值
    assert!(
        text.contains("100") || text.contains("200"),
        "With showData, raw values should appear"
    );
}

#[test]
fn pie_diagram_with_single_sector() {
    let svg = render_to_svg("pie\n\"Only\": 100", 400, 400);
    let text = strip_xml(&svg);

    let path_count = svg.matches("<path ").count();
    assert_eq!(path_count, 1, "Single sector pie should have 1 path");

    // 单个扇区应显示 100.0%
    assert!(text.contains("100.0%"), "Should show 100%");
}

#[test]
fn pie_diagram_validates_color_count() {
    let svg = render_to_svg("pie\nA: 10\nB: 20\nC: 30\nD: 40\nE: 50\nF: 60", 600, 600);

    let path_count = svg.matches("<path ").count();
    assert_eq!(
        path_count, 6,
        "6 data entries should produce 6 path elements"
    );
}

// ============================================================
// Flowchart 图表测试
// ============================================================

#[test]
fn flowchart_with_two_nodes_renders_elements() {
    let svg = render_to_svg("flowchart TD\nA --> B", 600, 400);

    // 1. SVG 结构
    assert!(svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg""#));
    assert!(svg.ends_with("</svg>\n"));

    // 2. 语义断言（不做外部属性对齐）：
    //    lievisual 渲染器总是输出 1 个整画布背景 <rect>，再加 N 个节点矩形。
    //    故至少应有 2 个 <rect>（1 背景 + 1 节点），实际是 1 背景 + 2 节点矩形。
    let rect_count = svg.matches("<rect ").count();
    assert!(
        rect_count >= 2,
        "Flowchart with 2 nodes should have background rect + node rects, got {rect_count}"
    );

    // 3. 语义：1 条边。新管线边渲染为贝塞尔曲线 <path>（每条边 = 1 个 path）。
    let edge_path_count = svg.matches("<path ").count();
    assert_eq!(
        edge_path_count, 1,
        "Flowchart with 1 edge should have 1 path (bezier), got {edge_path_count}"
    );

    // 4. 语义：节点文本存在
    assert!(svg.contains("A"), "SVG should contain node label A");
    assert!(svg.contains("B"), "SVG should contain node label B");
}

#[test]
fn flowchart_with_three_chain_nodes() {
    // 注意：语法要求边之间用换行分隔
    let svg = render_to_svg("flowchart LR\nA --> B\nB --> C", 800, 400);

    // 语义：至少背景矩形 + 3 个节点矩形
    let rect_count = svg.matches("<rect ").count();
    assert!(
        rect_count >= 3,
        "Chain of 3 nodes should have background rect + 3 node rects, got {rect_count}"
    );

    // 语义：2 条边（新管线：每条边 = 1 个贝塞尔曲线 <path>）
    let edge_path_count = svg.matches("<path ").count();
    assert_eq!(
        edge_path_count, 2,
        "Chain of 3 nodes should have 2 path (bezier), got {edge_path_count}"
    );

    // 验证文本
    assert!(svg.contains("A"));
    assert!(svg.contains("B"));
    assert!(svg.contains("C"));
}

#[test]
fn flowchart_with_node_text() {
    // 注意：节点声明与边必须分行，带空格的文本需用引号括起
    let svg = render_to_svg(
        "flowchart TD\nA[\"Start\"]\nB[\"Process Data\"]\nC[\"End\"]\nA --> B\nB --> C",
        600,
        500,
    );
    let text = strip_xml(&svg);

    assert!(text.contains("Start"), "Should render node text 'Start'");
    assert!(
        text.contains("Process Data"),
        "Should render node text 'Process Data'"
    );
    assert!(text.contains("End"), "Should render node text 'End'");
}

#[test]
fn flowchart_uses_correct_dimensions() {
    let svg = render_to_svg("flowchart TD\nX --> Y", 800, 600);

    assert!(svg.contains("width=\"800.00\""), "SVG width should be 800");
    assert!(
        svg.contains("height=\"600.00\""),
        "SVG height should be 600"
    );
}

// ============================================================
// Timeline 图表测试（新管线 Linear 家族）
// ============================================================

#[test]
fn timeline_renders_sections_and_events() {
    let svg = render_to_svg(
        "timeline\ntitle History\nsection Early\n1900 : Born\nsection Later\n1950 : Retire",
        800,
        600,
    );
    let text = strip_xml(&svg);
    // 标题、section 名与事件文本都应渲染。
    assert!(text.contains("History"), "应渲染标题 History");
    assert!(text.contains("Early"), "应渲染 section 名 Early");
    assert!(text.contains("Later"), "应渲染 section 名 Later");
    assert!(text.contains("1900"), "应渲染事件 1900");
    assert!(text.contains("Born"), "应渲染事件 Born");
    assert!(text.contains("1950"), "应渲染事件 1950");
    assert!(text.contains("Retire"), "应渲染事件 Retire");
}

#[test]
fn timeline_without_title_renders() {
    let svg = render_to_svg("timeline\nsection A\n2000 : X", 600, 400);
    let text = strip_xml(&svg);
    assert!(text.contains("A"), "应渲染 section 名 A");
    assert!(text.contains("2000"), "应渲染事件 2000");
    assert!(text.contains("X"), "应渲染事件 X");
}

// ============================================================
// Sequence 图表测试（新管线 Sequence 家族）
// ============================================================

#[test]
fn sequence_renders_participants_messages_and_notes() {
    let svg = render_to_svg(
        "sequenceDiagram\n    participant A as Alice\n    participant B as Bob\n    A->>B: hi\n    Note over A,B: shared note\n    B-->>A: ack\n",
        800,
        600,
    );
    let text = strip_xml(&svg);
    assert!(text.contains("Alice"), "应渲染参与者 Alice（alias）");
    assert!(text.contains("Bob"), "应渲染参与者 Bob");
    assert!(text.contains("hi"), "应渲染消息 hi");
    assert!(text.contains("shared note"), "应渲染备注文本");
    assert!(text.contains("ack"), "应渲染消息 ack");
}

#[test]
fn sequence_renders_loop_block_label() {
    let svg = render_to_svg(
        "sequenceDiagram\n    A->>B: x\n    loop retry\n        A->>B: again\n        B-->>A: ack\n    end\n",
        800,
        600,
    );
    let text = strip_xml(&svg);
    assert!(text.contains("loop [retry]"), "应渲染分组块标签 loop [retry]");
    assert!(text.contains("again"), "应渲染块内消息 again");
    assert!(text.contains("ack"), "应渲染块内消息 ack");
}

// ============================================================
// GitGraph 图表测试（新管线 Hierarchy 家族）
// ============================================================

#[test]
fn gitgraph_renders_commit_ids_branches_and_dashed_lanes() {
    let svg = render_to_svg(
        "gitGraph:\n    commit\n    commit\n    branch develop\n    checkout develop\n    commit\n    commit\n    checkout main\n    merge develop\n    commit\n",
        900,
        500,
    );
    let text = strip_xml(&svg);
    // commit id 标签（除 merge 外）。
    assert!(text.contains("c0"), "应渲染 commit id c0");
    assert!(text.contains("c1"), "应渲染 commit id c1");
    assert!(text.contains("c2"), "应渲染 commit id c2");
    assert!(text.contains("c3"), "应渲染 commit id c3");
    assert!(text.contains("c5"), "应渲染 commit id c5");
    // merge 无显式标签时不应出现默认 "merge develop" 标签（对齐官方）。
    assert!(
        !text.contains("merge develop"),
        "无显式标签的 merge 不应显示默认标签"
    );
    // 分支标签。
    assert!(text.contains("main"), "应渲染分支标签 main");
    assert!(text.contains("develop"), "应渲染分支标签 develop");
    // 分支虚线（平行状态标识）。
    assert!(
        svg.contains("stroke-dasharray"),
        "分支行应有虚线标识"
    );
}

// ============================================================
// 渲染器功能测试
// ============================================================
