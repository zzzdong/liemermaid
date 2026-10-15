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
    // 画布语义对齐官方：响应式视口（根节点只写 viewBox）+ 贴合内容（600×400 为上限）。
    assert!(
        !svg.contains("<svg width=\""),
        "SVG 根节点不应写死 width（视口交给 CSS）"
    );
    assert!(svg.contains("viewBox="), "SVG 应带 viewBox");

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

    // 5. 扇区标签为**整数百分比**（官方 golden：`40%` / `35%` / `25%`）。
    assert!(text.contains("30%"), "SVG should show 30% for A");
    assert!(text.contains("50%"), "SVG should show 50% for B");
    assert!(text.contains("20%"), "SVG should show 20% for C");

    // 6. 图例：官方在饼图右侧画色块 + 名称（名称不再贴在扇区外侧）。
    assert!(
        svg.contains("<ellipse"),
        "官方 pie 有外圈（`<circle class=\"pieOuterCircle\">`，此处以 ellipse 输出）"
    );
    // 图例色块为 18×18 实心矩形（与扇区同色）。
    assert!(
        svg.contains("width=\"18.00\" height=\"18.00\""),
        "图例色块应为 18×18"
    );
    // 7. 扇区分隔线：官方 `.pieCircle{stroke:black;stroke-width:2px}`，
    //    黑描边才能在相邻浅色扇区之间看清切分边界。
    assert!(
        svg.matches("stroke=\"#000000\"").count() >= 4,
        "3 个扇区 + 外圈均应有黑色描边"
    );
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

    // 单个扇区应显示 100%（官方整数百分比）
    assert!(text.contains("100%"), "Should show 100%");
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

/// 边标签背景按官方各图类型 `.labelBkg` 规则（golden 实测）：
/// - flowchart：`rgba(232,232,232,0.5)` 半透明浅灰
/// - ER：`rgba(248.67,255,236,0.5)` 半透明淡黄绿
/// - class：`#ECECFF` 不透明
/// - state：**无 labelBkg 规则 → 不画背景**（旧实现统一纯白，三处皆偏）
#[test]
fn edge_label_background_matches_official_per_type() {
    // flowchart：半透明 #e8e8e8。
    let fc = render_to_svg("flowchart TD\nD{?} -->|one| A\nD -->|two| B", 800, 600);
    // alpha 128/255 在 SVG 中输出为 fill-opacity="0.502"（≈0.5）。
    assert!(
        fc.contains(r##"fill="#e8e8e8""##) && fc.contains(r##"fill-opacity="0.5"##),
        "flowchart 边标签应为 #e8e8e8 @0.5: {fc}"
    );
    // ER：半透明 #f8ffec。
    let er = render_to_svg("erDiagram\nCUSTOMER ||--o{ ORDER : places", 800, 600);
    assert!(
        er.contains(r##"fill="#f8ffec""##),
        "ER 边标签应为 #f8ffec @0.5: {er}"
    );
    // class：不透明 #ECECFF。
    let cl = render_to_svg("classDiagram\nclass A\nclass B\nA --> B : uses", 800, 600);
    assert!(
        cl.contains(r##"fill="#ececff""##),
        "class 边标签应为不透明 #ECECFF: {cl}"
    );
    // state：无背景矩形（官方 state golden 无 .labelBkg 规则）。
    let st = render_to_svg(
        "stateDiagram-v2\n[*] --> Idle : go\nIdle --> Done : done",
        800,
        600,
    );
    assert!(
        !st.contains("edge-label"),
        "state 边标签不应有背景框（官方透明）: {st}"
    );
    // 标签文本本身仍要渲染。
    for (svg, needle) in [(&fc, "one"), (&er, "places"), (&cl, "uses"), (&st, "go")] {
        assert!(strip_xml(svg).contains(needle), "应渲染标签文本 {needle}");
    }
}

/// 画布语义对齐官方 mermaid：`viewBox` 贴合内容、视口交给 CSS（`width` / `height`
/// 不写死），`config` 的 800×600 是**上限**而非固定画布（内容更小则不撑满）。
#[test]
fn flowchart_canvas_fits_content_like_official() {
    let svg = render_to_svg("flowchart TD\nX --> Y", 800, 600);
    assert!(
        !svg.contains("<svg width=\""),
        "官方 SVG 根节点不写死 width"
    );
    assert!(
        svg.contains("max-width:"),
        "官方根节点带 max-width 样式（实际渲染宽度）"
    );
    // 解析 viewBox，校验画布贴合内容且不超出请求上限 800×600。
    let vb = svg
        .split_once("viewBox=\"")
        .and_then(|(_, s)| s.split_once('"'))
        .map(|(s, _)| s.to_string())
        .expect("应存在 viewBox");
    let nums: Vec<f64> = vb
        .split_whitespace()
        .map(|n| n.parse().expect("viewBox 数字"))
        .collect();
    assert_eq!(nums.len(), 4, "viewBox 应为 4 个数: {vb}");
    let (w, h) = (nums[2], nums[3]);
    assert!(w > 0.0 && h > 0.0, "画布应有正尺寸: {vb}");
    assert!(w <= 800.0 + 20.0, "画布宽不应超出请求上限: {w}");
    assert!(h <= 600.0 + 20.0, "画布高不应超出请求上限: {h}");
    // 两个小节点的内容远小于 800×600，画布应贴合而非撑满。
    assert!(w < 400.0, "画布应贴合内容而非撑满: {w}");
}

/// 内容大于配置上限时**等比缩小**（绝不放大、绝不裁切）。
#[test]
fn oversized_content_is_scaled_down() {
    let wide = render_to_svg(
        "flowchart LR\nA --> B --> C --> D --> E --> F --> G --> H",
        300,
        300,
    );
    let vb = wide
        .split_once("viewBox=\"")
        .and_then(|(_, s)| s.split_once('"'))
        .map(|(s, _)| s.to_string())
        .expect("viewBox");
    let nums: Vec<f64> = vb.split_whitespace().map(|n| n.parse().unwrap()).collect();
    assert!(nums[2] <= 300.0 + 20.0, "超出上限应被缩小: {vb}");
    assert!(nums[3] <= 300.0 + 20.0, "超出上限应被缩小: {vb}");
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

/// timeline 结构对齐官方 golden `timeline__basic`（实测）：
/// - period 块（`events[0]`，如 1900）在主轴**上方**，event 块在轴下方；
/// - 轴上**无圆点**（官方无 circle）；
/// - 每列一条 2px 黑虚线（dash 5,5）连接 period 与 event；
/// - 块为顶角圆、底角直角的彩色多边形。
#[test]
fn timeline_matches_official_structure() {
    let svg = render_to_svg(
        "timeline\ntitle H\nsection A\n1900 : Born\nsection B\n1950 : Retire",
        800,
        600,
    );
    // 轴上无圆点：官方 golden 无 circle/ellipse。
    assert!(
        !svg.contains("<ellipse") && !svg.contains("<circle"),
        "官方 timeline 轴上无圆点: {svg}"
    );
    // 连接虚线：2px、dash 5,5、黑色。
    assert!(
        svg.contains(r##"stroke-width="2.00""##)
            && svg.contains(r##"stroke-dasharray="5.00,5.00""##),
        "应有 2px dash 5,5 的连接虚线"
    );
    // 主轴 4px 黑色。
    assert!(
        svg.contains(r##"stroke-width="4.00""##),
        "主轴应为 4px（官方 stroke-width=4）"
    );
    // period 1900 的 y 应在主轴（y=0 附近，transform 平移后取相对坐标）上方，
    // event Born 在下方：用文本 y 坐标比较（1900 的 y < Born 的 y）。
    let y_of = |needle: &str| -> f64 {
        let tag = format!(">{needle}</text>");
        let idx = svg.find(&tag).expect(needle);
        // 向前找该 text 元素的起始标签，再在其属性区内找 y 属性
        // （不能全局 rfind "y=\""——`font-family="..."` 里也含 `y="`）。
        let head = &svg[..idx];
        let start = head.rfind("<text ").expect("text start");
        let attrs = &head[start..];
        let ypos = attrs.find(" y=\"").expect("y attr");
        let rest = &attrs[ypos + 4..];
        let end = rest.find('"').unwrap();
        rest[..end].parse().unwrap()
    };
    let y_period = y_of("1900");
    let y_event = y_of("Born");
    assert!(
        y_period < y_event,
        "period 1900 (y={y_period}) 应在 event Born (y={y_event}) 上方"
    );
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
    // 官方把块标签拆成两段文本：`loop` + `[retry]`。
    assert!(text.contains("loop"), "应渲染分组块关键字 loop");
    assert!(text.contains("[retry]"), "应渲染分组块描述 [retry]");
    assert!(text.contains("again"), "应渲染块内消息 again");
    assert!(text.contains("ack"), "应渲染块内消息 ack");
}

#[test]
fn sequence_renders_activation_bars() {
    // `A->>+B` 激活目标 B，`B-->>-A` 取消激活**源** B（官方语义）。
    let svg = render_to_svg(
        "sequenceDiagram\n    participant A\n    participant B\n    A->>+B: start\n    B-->>-A: done\n",
        800,
        600,
    );
    // 官方激活条计算样式（浏览器实测 golden）：`.activation0{fill:#f4f4f4;stroke:#666}`
    // ——内联 `fill="#EDF2AE"` 被 CSS 覆盖，真实呈现为 #f4f4f4。
    assert!(
        svg.contains(r##"fill="#f4f4f4""##),
        "应渲染激活条（官方 activation 计算样式 #f4f4f4）"
    );
    // 激活条宽 10、有边框。
    let act = svg
        .lines()
        .find(|l| l.contains(r##"fill="#f4f4f4""##))
        .expect("存在激活条矩形");
    assert!(
        act.contains(r##"width="10.00""##),
        "激活条宽度应为 10: {act}"
    );
    // 激活条纵向跨度应大于一行（跨越 start → done 两行）。
    let h = act
        .split(r##"height=""##)
        .nth(1)
        .and_then(|s| s.split('"').next())
        .and_then(|s| s.parse::<f64>().ok())
        .expect("激活条高度");
    assert!(h >= 40.0, "激活条应跨越多行: h={h}");
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
    // commit id 标签：官方无显式 id 时为 `<序号>-<7位十六进制>` 自动哈希
    // （序号 = 提交序，merge 不占标签），此处断言同格式。
    let re = |needle: &str| text.contains(needle);
    assert!(re("0-"), "应渲染 0 号提交的哈希标签");
    assert!(re("1-"), "应渲染 1 号提交的哈希标签");
    assert!(re("2-"), "应渲染 2 号提交的哈希标签");
    assert!(re("3-"), "应渲染 3 号提交的哈希标签");
    assert!(re("5-"), "应渲染 5 号提交的哈希标签（merge 4 无标签）");
    // merge 无显式标签时不应出现默认 "merge develop" 标签（对齐官方）。
    assert!(
        !text.contains("merge develop"),
        "无显式标签的 merge 不应显示默认标签"
    );
    // 分支标签。
    assert!(text.contains("main"), "应渲染分支标签 main");
    assert!(text.contains("develop"), "应渲染分支标签 develop");
    // 分支虚线（平行状态标识）。
    assert!(svg.contains("stroke-dasharray"), "分支行应有虚线标识");
}

// ============================================================
// 渲染器功能测试
// ============================================================
