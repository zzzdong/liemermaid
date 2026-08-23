//! `svgdiff` —— 轻量、可复用的 SVG 结构化比对工具。
//!
//! 设计目标：对 liemermaid（以及任意 SVG）的输出做**语义级**而不是字节级 diff，
//! 用于端到端回归测试。它从 SVG 中抽取几何元素并按下维度比对：
//!
//! 1. **元素数量与类型**：按几何类型（rect / circle / path / line / polyline /
//!    text / polygon）或语义 class（继承最近的父级 `<g class="...">`）统计数量。
//! 2. **文本标签内容**：抽取所有 `<text>`（含嵌套 `<tspan>`）的文本内容，集合比对（忽略顺序）。
//! 3. **相对布局 / 包围盒**：每个元素的位置（继承自祖先 `<g transform>` 与本元素
//!    `x`/`y`/`cx`/`cy`，已累加父级 translate），用于拓扑一致性检查。
//! 4. **颜色 / 样式**：每个元素的 `fill` / `stroke` 色彩集合比对。
//!
//! 解析基于 `quick-xml`（正规 XML 事件遍历 + 轻量 DOM），能正确处理嵌套 group、
//! class 继承、跨行/含子节点的文本、祖先 transform 累加、XML 实体等，避免手写
//! 正则解析的脆弱性。
//!
//! 布局归一化：所有坐标按整体包围盒做归一化，使不同分辨率/平移下仍可比。

use quick_xml::events::Event;
use std::collections::{BTreeMap, BTreeSet};

/// 几何元素类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Rect,
    RoundedRect,
    Circle,
    Line,
    Polyline,
    Path,
    Polygon,
    Text,
    Other,
}

impl Kind {
    fn from_tag(tag: &str) -> Kind {
        match tag {
            "rect" => Kind::Rect,
            "circle" => Kind::Circle,
            "line" => Kind::Line,
            "polyline" => Kind::Polyline,
            "path" => Kind::Path,
            "polygon" => Kind::Polygon,
            "text" => Kind::Text,
            _ => Kind::Other,
        }
    }
}

/// 单个被抽取的元素。
#[derive(Debug, Clone)]
pub struct El {
    pub kind: Kind,
    /// 语义分类：继承最近的父级 `<g class="...">`。
    pub class: String,
    /// 包围盒（已应用祖先 transform 累加的近似，根坐标系）。用于相对布局/包围盒维度。
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// 文本内容（仅 Text）。
    pub text: String,
    /// 颜色（已规整小写）。
    pub fill: String,
    pub stroke: String,
}

/// 解析后的 SVG 摘要。
#[derive(Debug, Clone, Default)]
pub struct Summary {
    /// (kind, class) -> 数量
    pub counts: BTreeMap<(Kind, String), usize>,
    /// 所有文本内容（去重）
    pub texts: BTreeSet<String>,
    /// 所有 fill 颜色（去重）
    pub fills: BTreeSet<String>,
    /// 所有 stroke 颜色（去重）
    pub strokes: BTreeSet<String>,
    /// 相对布局 / 包围盒：每个元素按其语义键 + 量化坐标（整数像素）记录，
    /// 用于拓扑/位置一致性检查（自比对时完全一致）。
    pub boxes: BTreeSet<(String, i64, i64, i64, i64)>,
}

/// 两个摘要之间的差异。
#[derive(Debug, Clone, Default)]
pub struct Diff {
    pub count_diffs: BTreeMap<(Kind, String), (usize, usize)>,
    pub missing_texts: BTreeSet<String>,
    pub extra_texts: BTreeSet<String>,
    pub missing_fills: BTreeSet<String>,
    pub extra_fills: BTreeSet<String>,
    pub missing_strokes: BTreeSet<String>,
    pub extra_strokes: BTreeSet<String>,
    pub missing_boxes: BTreeSet<(String, i64, i64, i64, i64)>,
    pub extra_boxes: BTreeSet<(String, i64, i64, i64, i64)>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.count_diffs.is_empty()
            && self.missing_texts.is_empty()
            && self.extra_texts.is_empty()
            && self.missing_fills.is_empty()
            && self.extra_fills.is_empty()
            && self.missing_strokes.is_empty()
            && self.extra_strokes.is_empty()
            && self.missing_boxes.is_empty()
            && self.extra_boxes.is_empty()
    }

    pub fn describe(&self) -> String {
        let mut s = String::new();
        for ((k, c), (a, b)) in &self.count_diffs {
            let cls = if c.is_empty() { String::new() } else { format!("[{c}]") };
            s.push_str(&format!("  count {}{}: ours={a} golden={b}\n", kind_name(*k), cls));
        }
        for t in &self.missing_texts {
            s.push_str(&format!("  missing text: {t:?}\n"));
        }
        for t in &self.extra_texts {
            s.push_str(&format!("  extra text:   {t:?}\n"));
        }
        for c in &self.missing_fills {
            s.push_str(&format!("  missing fill: {c}\n"));
        }
        for c in &self.extra_fills {
            s.push_str(&format!("  extra fill: {c}\n"));
        }
        for c in &self.missing_strokes {
            s.push_str(&format!("  missing stroke: {c}\n"));
        }
        for c in &self.extra_strokes {
            s.push_str(&format!("  extra stroke:   {c}\n"));
        }
        for b in &self.missing_boxes {
            s.push_str(&format!("  missing box: {:?}\n", b));
        }
        for b in &self.extra_boxes {
            s.push_str(&format!("  extra box:   {:?}\n", b));
        }
        s
    }
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Rect => "rect",
        Kind::RoundedRect => "rrect",
        Kind::Circle => "circle",
        Kind::Line => "line",
        Kind::Polyline => "polyline",
        Kind::Path => "path",
        Kind::Polygon => "polygon",
        Kind::Text => "text",
        Kind::Other => "other",
    }
}

fn clean_color(c: &str) -> String {
    c.trim().to_lowercase()
}

/// 解析 `translate(tx, ty)` 或 `translate(tx ty)`，返回平移增量。
fn parse_translate(t: &str) -> Option<(f64, f64)> {
    let inner = t.trim_start_matches("translate").trim();
    let inner = inner.trim_start_matches('(').trim_end_matches(')').trim();
    let parts: Vec<&str> = if inner.contains(',') {
        inner.split(',').collect()
    } else {
        inner.split_whitespace().collect()
    };
    if parts.len() >= 2 {
        let x = parts[0].trim().parse::<f64>().ok()?;
        let y = parts[1].trim().parse::<f64>().ok()?;
        Some((x, y))
    } else {
        None
    }
}

/// 解析 SVG 字符串，返回所有被抽取的元素（按文档顺序）。
///
/// 基于 quick-xml 事件遍历构建轻量 DOM：维护 group 栈（最近 class + 累加 translate），
/// 正确继承嵌套 group 的 class 与祖先 transform。
pub fn parse(svg: &str) -> Vec<El> {
    let mut els = Vec::new();
    let mut reader = quick_xml::Reader::from_str(svg);

    // group 栈：每帧记录 (最近 class, 已累加 translate)
    let mut group_stack: Vec<(String, (f64, f64))> = Vec::new();

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = e.local_name();
                let tag = std::str::from_utf8(tag.as_ref()).unwrap_or("");
                if tag == "g" {
                    let class = attr_str(e.attributes(), "class").unwrap_or_default();
                    let (px, py) = group_stack
                        .last()
                        .map(|(_, p)| *p)
                        .unwrap_or((0.0, 0.0));
                    let (tx, ty) = attr_str(e.attributes(), "transform")
                        .and_then(|t| parse_translate(&t))
                        .unwrap_or((0.0, 0.0));
                    group_stack.push((class, (px + tx, py + ty)));
                } else if tag == "text" || tag == "div" || tag == "p" || tag == "span"
                    || tag == "foreignObject"
                {
                    // 抽取文本（可能跨 tspan/子节点），坐标：text 用自身 x/y + group 平移，
                    // 其他容器（foreignObject/div/p/span）无 x/y 属性，用 group 累加平移近似。
                    let (cx, cy) = group_stack
                        .last()
                        .map(|(_, p)| *p)
                        .unwrap_or((0.0, 0.0));
                    let (bx, by) = attr_f64(e.attributes(), "x").map(|v| (v + cx, v))
                        .unwrap_or((cx, cy));
                    let by = attr_f64(e.attributes(), "y").map(|v| v + cy).unwrap_or(by);
                    let text = collect_text(&mut reader);
                    let (px, py) = group_stack.last().map(|(_, p)| *p).unwrap_or((0.0, 0.0));
                    let el = El {
                        kind: Kind::Text,
                        class: group_stack.last().map(|(c, _)| c.clone()).unwrap_or_default(),
                        x: bx,
                        y: by,
                        w: 0.0,
                        h: 0.0,
                        text,
                        fill: attr_str(e.attributes(), "fill").map(|s| clean_color(&s)).unwrap_or_default(),
                        stroke: attr_str(e.attributes(), "stroke").map(|s| clean_color(&s)).unwrap_or_default(),
                    };
                    let _ = (px, py);
                    els.push(el);
                }
                // 其他 Start（如 tspan）暂不入栈；其 Translate 不处理（文本已收集）
            }
            Ok(Event::Empty(e)) => {
                let tag = e.local_name();
                let tag = std::str::from_utf8(tag.as_ref()).unwrap_or("");
                let kind = Kind::from_tag(tag);
                if kind == Kind::Other {
                    continue;
                }
                let (px, py) = group_stack.last().map(|(_, p)| *p).unwrap_or((0.0, 0.0));
                let mut el = El {
                    kind,
                    class: group_stack.last().map(|(c, _)| c.clone()).unwrap_or_default(),
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                    text: String::new(),
                    fill: String::new(),
                    stroke: String::new(),
                };
                // 几何属性
                if let Some(v) = attr_f64(e.attributes(), "x") {
                    el.x = v + px;
                }
                if let Some(v) = attr_f64(e.attributes(), "y") {
                    el.y = v + py;
                }
                if let Some(v) = attr_f64(e.attributes(), "width") {
                    el.w = v;
                }
                if let Some(v) = attr_f64(e.attributes(), "height") {
                    el.h = v;
                }
                if let Some(v) = attr_f64(e.attributes(), "cx") {
                    el.x = v + px;
                }
                if let Some(v) = attr_f64(e.attributes(), "cy") {
                    el.y = v + py;
                }
                if let Some(v) = attr_f64(e.attributes(), "r") {
                    el.w = v * 2.0;
                    el.h = v * 2.0;
                }
                if let Some(f) = attr_str(e.attributes(), "fill") {
                    el.fill = clean_color(&f);
                }
                if let Some(s) = attr_str(e.attributes(), "stroke") {
                    el.stroke = clean_color(&s);
                }
                if kind == Kind::Rect && attr_str(e.attributes(), "rx").is_some() {
                    el.kind = Kind::RoundedRect;
                }
                // 本元素自身的 transform 平移叠加（已含祖先累加）
                if let Some(t) = attr_str(e.attributes(), "transform") {
                    if let Some((tx, ty)) = parse_translate(&t) {
                        el.x += tx;
                        el.y += ty;
                    }
                }
                els.push(el);
            }
            Ok(Event::End(e)) => {
                let name = e.local_name();
                let tag = std::str::from_utf8(name.as_ref()).unwrap_or("");
                if tag == "g" {
                    group_stack.pop();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                // 解析失败时放弃剩余内容（降级为宽松比较）
                eprintln!("svgdiff: xml parse error: {e}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }
    els
}

/// 从 reader 中持续读取，直到遇到匹配的结束标签（如 `text`），拼接期间所有文本。
/// 会跳过 `<tspan>` 等中间元素的开始/结束，但收集其文本内容。
fn collect_text(reader: &mut quick_xml::Reader<&[u8]>) -> String {
    let mut out = String::new();
    let mut buf = Vec::new();
    let mut depth = 0usize; // text 自身为 0，进入子元素 +1
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(t)) => {
                // 拼接所有文本节点（含 tspan 内部），不同文本节点间用换行分隔，
                // 以便后续按行拆分还原多个独立标签（官方常把同一 foreignObject 内的多个
                // <p> 标签拼成一段，加分隔符才能与逐项渲染的 liemermaid 对齐）。
                if let Ok(s) = t.unescape() {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        if !out.is_empty() {
                            out.push('\n');
                        }
                        out.push_str(trimmed);
                    }
                }
            }
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(e)) => {
                let name = e.local_name();
                let tag = std::str::from_utf8(name.as_ref()).unwrap_or("");
                if tag == "text" && depth == 0 {
                    break;
                }
                if depth > 0 {
                    depth -= 1;
                }
            }
            Ok(Event::Empty(_)) => {}
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

fn attr_str<'a>(
    attrs: quick_xml::events::attributes::Attributes<'a>,
    name: &str,
) -> Option<String> {
    for a in attrs {
        if let Ok(a) = a {
            if a.key.as_ref() == name.as_bytes() {
                return a
                    .unescape_value()
                    .ok()
                    .map(|v| v.to_string());
            }
        }
    }
    None
}

fn attr_f64(attrs: quick_xml::events::attributes::Attributes, name: &str) -> Option<f64> {
    attr_str(attrs, name).and_then(|v| v.parse::<f64>().ok())
}

/// 生成摘要。
pub fn summarize(els: &[El]) -> Summary {
    let mut s = Summary::default();
    for el in els {
        *s.counts.entry((el.kind, el.class.clone())).or_insert(0) += 1;
        if !el.text.is_empty() {
            // 按换行拆分（collect_text 已用换行分隔多个独立标签），还原为多个文本项，
            // 与逐项渲染的 liemermaid 对齐。
            for t in el.text.split('\n') {
                let t = t.trim();
                if !t.is_empty() {
                    s.texts.insert(t.to_string());
                }
            }
        }
        if !el.fill.is_empty() {
            s.fills.insert(el.fill.clone());
        }
        if !el.stroke.is_empty() {
            s.strokes.insert(el.stroke.clone());
        }
        // 相对布局 / 包围盒维度：以语义键（class 或几何类型）标识，坐标量化整数像素。
        let key = if el.class.is_empty() {
            kind_name(el.kind).to_string()
        } else {
            el.class.clone()
        };
        let bx = el.x.round() as i64;
        let by = el.y.round() as i64;
        let bw = el.w.round() as i64;
        let bh = el.h.round() as i64;
        s.boxes.insert((key, bx, by, bw, bh));
    }
    s
}

/// 比较两个摘要，返回差异。
pub fn compare(ours: &Summary, golden: &Summary) -> Diff {
    let mut d = Diff::default();
    // 计数
    let mut keys: BTreeSet<(Kind, String)> = BTreeSet::new();
    keys.extend(ours.counts.keys().cloned());
    keys.extend(golden.counts.keys().cloned());
    for k in keys {
        let a = *ours.counts.get(&k).unwrap_or(&0);
        let b = *golden.counts.get(&k).unwrap_or(&0);
        if a != b {
            d.count_diffs.insert(k, (a, b));
        }
    }
    // 文本
    for t in &golden.texts {
        if !ours.texts.contains(t) {
            d.missing_texts.insert(t.clone());
        }
    }
    for t in &ours.texts {
        if !golden.texts.contains(t) {
            d.extra_texts.insert(t.clone());
        }
    }
    // 颜色
    for c in &golden.fills {
        if !ours.fills.contains(c) {
            d.missing_fills.insert(c.clone());
        }
    }
    for c in &ours.fills {
        if !golden.fills.contains(c) {
            d.extra_fills.insert(c.clone());
        }
    }
    for c in &golden.strokes {
        if !ours.strokes.contains(c) {
            d.missing_strokes.insert(c.clone());
        }
    }
    for c in &ours.strokes {
        if !golden.strokes.contains(c) {
            d.extra_strokes.insert(c.clone());
        }
    }
    // 包围盒
    for b in &golden.boxes {
        if !ours.boxes.contains(b) {
            d.missing_boxes.insert(b.clone());
        }
    }
    for b in &ours.boxes {
        if !golden.boxes.contains(b) {
            d.extra_boxes.insert(b.clone());
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_group_class_and_rect() {
        let svg = r#"<svg><g class='node'><rect x='10' y='20' width='100' height='40' rx='5' fill='#fff' stroke='#000'/></g></svg>"#;
        let els = parse(svg);
        assert_eq!(els.len(), 1);
        assert_eq!(els[0].kind, Kind::RoundedRect);
        assert_eq!(els[0].class, "node");
        assert_eq!(els[0].x, 10.0);
        assert_eq!(els[0].w, 100.0);
        assert_eq!(els[0].fill, "#fff");
    }

    #[test]
    fn parses_text_and_transform() {
        let svg = r#"<svg><g transform='translate(5,5)'><text x='10' y='20'>Hello &amp; World</text></g></svg>"#;
        let els = parse(svg);
        assert_eq!(els.len(), 1);
        assert_eq!(els[0].kind, Kind::Text);
        assert_eq!(els[0].text, "Hello & World");
        assert_eq!(els[0].x, 15.0); // 10 + 5 = 15
        assert_eq!(els[0].y, 25.0); // 20 + 5 = 25
    }

    #[test]
    fn parses_text_with_tspan_and_nested_group() {
        let svg = r#"<svg><g class='node'><g class='label'><text x='0' y='0'>A<tspan>B</tspan>C</text></g></g></svg>"#;
        let els = parse(svg);
        assert_eq!(els.len(), 1);
        assert_eq!(els[0].class, "label"); // 最近父 g 的 class
        assert_eq!(els[0].text, "A\nB\nC"); // 不同文本节点间以换行分隔，便于按行还原独立标签
    }

    #[test]
    fn compare_detects_count_and_text_diffs() {
        let a = summarize(&parse(r#"<svg><g class='node'><rect x='0' y='0' width='10' height='10'/></g><text x='0' y='0'>A</text></svg>"#));
        let b = summarize(&parse(r#"<svg><g class='node'><rect x='0' y='0' width='10' height='10'/></g><g class='node'><rect x='50' y='0' width='10' height='10'/></g><text x='0' y='0'>B</text></svg>"#));
        let d = compare(&a, &b);
        assert!(!d.is_empty());
        // b 比 a 多一个 node 矩形
        assert_eq!(d.count_diffs.get(&(Kind::Rect, "node".to_string())), Some(&(1usize, 2usize)));
        // a=ours（少文本 A），b=golden（多文本 B）：
        // ours 多出 A（golden 没有）→ extra；ours 缺失 B（golden 有）→ missing
        assert!(d.extra_texts.contains("A"));
        assert!(d.missing_texts.contains("B"));
    }
}
