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

    // 2. 应该有 2 个节点矩形（SVG 背景透明，不再绘制整画布背景矩形，与官方 mermaid 对齐）
    let rect_count = svg.matches("<rect ").count();
    assert_eq!(
        rect_count, 2,
        "Flowchart with 2 nodes should have 2 rect elements (no background rect)"
    );

    // 3. 应该有 1 条边（liemermaid 用 <path class="edge"> 表示边，与官方 edgePaths 等价）
    let edge_count = svg.matches("class=\"edge\"").count();
    assert_eq!(
        edge_count, 1,
        "Flowchart with 1 edge should have one edge path"
    );

    // 4. 文本内容
    assert!(svg.contains("A"), "SVG should contain node label A");
    assert!(svg.contains("B"), "SVG should contain node label B");
}

#[test]
fn flowchart_with_three_chain_nodes() {
    // 注意：语法要求边之间用换行分隔
    let svg = render_to_svg("flowchart LR\nA --> B\nB --> C", 800, 400);

    let rect_count = svg.matches("<rect ").count();
    assert_eq!(
        rect_count, 3,
        "Chain of 3 nodes should have 3 rectangles (no background rect)"
    );

    let edge_count = svg.matches("class=\"edge\"").count();
    assert_eq!(
        edge_count, 2,
        "Chain of 3 nodes should have exactly 2 edges"
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
// 渲染器功能测试
// ============================================================
