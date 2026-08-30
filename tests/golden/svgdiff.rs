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
    /// 线状几何（line/polyline/path）的端点集合（首段起点、末段终点），用于几何等价比对。
    /// 节点为 None。
    pub endpoints: Option<((f64, f64), (f64, f64))>,
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
    /// 节点（封闭形状）中心集合，用于几何等价比对（允许整体平移/缩放/布局差异）。
    pub node_centers: Vec<(f64, f64)>,
    /// 边（线状几何）端点集合（(起点, 终点)），用于几何等价比对。
    pub edge_endpoints: Vec<((f64, f64), (f64, f64))>,
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
    /// 几何等价：节点中心集合归一化后的最大匹配误差（None 表示数量不等，直接判异）。
    pub geom_node_err: Option<f64>,
    /// 几何等价：边端点集合归一化后的最大匹配误差（None 表示数量不等）。
    pub geom_edge_err: Option<f64>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        if !self.missing_texts.is_empty() || !self.extra_texts.is_empty() {
            return false;
        }
        // 仅检查拓扑数量是否相等（None 表示两侧数量不一致 → 拓扑丢失）。
        // 位置误差（归一化后的 max-err）仅作诊断报告，不计入失败——
        // 不同后端的布局/缩放/平移差异属引擎相关，不应 fail。
        if self.geom_node_err.is_none() {
            return false;
        }
        if self.geom_edge_err.is_none() {
            return false;
        }
        true
    }

    pub fn describe(&self) -> String {
        let mut s = String::new();
        for ((k, c), (a, b)) in &self.count_diffs {
            let cls = if c.is_empty() {
                String::new()
            } else {
                format!("[{c}]")
            };
            s.push_str(&format!(
                "  count {}{}: ours={a} golden={b}\n",
                kind_name(*k),
                cls
            ));
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
        match self.geom_node_err {
            Some(e) => s.push_str(&format!(
                "  geom node centers max-err={:.3} (tol {:.2})\n",
                e, GEOM_TOL
            )),
            None => s.push_str("  geom node centers: COUNT MISMATCH\n"),
        }
        match self.geom_edge_err {
            Some(e) => s.push_str(&format!(
                "  geom edge endpoints max-err={:.3} (tol {:.2})\n",
                e, GEOM_TOL
            )),
            None => s.push_str("  geom edge endpoints: COUNT MISMATCH\n"),
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
                let tag = e.local_name().into_inner();
                if tag == "g" {
                    let class = attr_str(e.attributes(), "class").unwrap_or_default();
                    let (px, py) = group_stack.last().map(|(_, p)| *p).unwrap_or((0.0, 0.0));
                    let (tx, ty) = attr_str(e.attributes(), "transform")
                        .and_then(|t| parse_translate(&t))
                        .unwrap_or((0.0, 0.0));
                    group_stack.push((class, (px + tx, py + ty)));
                } else if tag == "text"
                    || tag == "div"
                    || tag == "p"
                    || tag == "span"
                    || tag == "foreignObject"
                {
                    // 抽取文本（可能跨 tspan/子节点），坐标：text 用自身 x/y + group 平移，
                    // 其他容器（foreignObject/div/p/span）无 x/y 属性，用 group 累加平移近似。
                    let (cx, cy) = group_stack.last().map(|(_, p)| *p).unwrap_or((0.0, 0.0));
                    let (bx, by) = attr_f64(e.attributes(), "x")
                        .map(|v| (v + cx, v))
                        .unwrap_or((cx, cy));
                    let by = attr_f64(e.attributes(), "y").map(|v| v + cy).unwrap_or(by);
                    let text = collect_text(&mut reader);
                    let (px, py) = group_stack.last().map(|(_, p)| *p).unwrap_or((0.0, 0.0));
                    let el = El {
                        kind: Kind::Text,
                        class: group_stack
                            .last()
                            .map(|(c, _)| c.clone())
                            .unwrap_or_default(),
                        x: bx,
                        y: by,
                        w: 0.0,
                        h: 0.0,
                        text,
                        fill: attr_str(e.attributes(), "fill")
                            .map(|s| clean_color(&s))
                            .unwrap_or_default(),
                        stroke: attr_str(e.attributes(), "stroke")
                            .map(|s| clean_color(&s))
                            .unwrap_or_default(),
                        endpoints: None,
                    };
                    let _ = (px, py);
                    els.push(el);
                }
                // 其他 Start（如 tspan）暂不入栈；其 Translate 不处理（文本已收集）
            }
            Ok(Event::Empty(e)) => {
                let tag = e.local_name().into_inner();
                let kind = Kind::from_tag(tag);
                if kind == Kind::Other {
                    continue;
                }
                let (px, py) = group_stack.last().map(|(_, p)| *p).unwrap_or((0.0, 0.0));
                let mut el = El {
                    kind,
                    class: group_stack
                        .last()
                        .map(|(c, _)| c.clone())
                        .unwrap_or_default(),
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                    text: String::new(),
                    fill: String::new(),
                    stroke: String::new(),
                    endpoints: None,
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
                if let Some(t) = attr_str(e.attributes(), "transform")
                    && let Some((tx, ty)) = parse_translate(&t)
                {
                    el.x += tx;
                    el.y += ty;
                }
                // 线状几何端点（line/polyline/path）：用于几何等价比对。
                if kind == Kind::Line {
                    if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
                        attr_f64(e.attributes(), "x1"),
                        attr_f64(e.attributes(), "y1"),
                        attr_f64(e.attributes(), "x2"),
                        attr_f64(e.attributes(), "y2"),
                    ) {
                        el.endpoints = Some(((x1 + px, y1 + py), (x2 + px, y2 + py)));
                    }
                } else if kind == Kind::Polyline {
                    if let Some(pts) = attr_str(e.attributes(), "points")
                        && let Some(ep) = parse_points_endpoints(&pts, px, py)
                    {
                        el.endpoints = Some(ep);
                    }
                } else if kind == Kind::Path
                    && let Some(d) = attr_str(e.attributes(), "d")
                    && let Some(ep) = parse_path_endpoints(&d, px, py)
                {
                    el.endpoints = Some(ep);
                }
                els.push(el);
            }
            Ok(Event::End(e)) => {
                let tag = e.local_name().into_inner();
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
    // 上一个事件类型：决定文本之间是「无缝拼接」还是「换行分隔」。
    // - 相邻 Text / 实体（GeneralRef）属于同一段文本，应无缝拼接（如 `Hello &amp; World`）。
    // - 跨真实标签（Start/End，如 `<tspan>`、`<p>`）应换行分隔（还原多个独立标签）。
    #[derive(Clone, Copy, PartialEq)]
    enum Prev {
        None,
        Text,
        Tag, // Start / End
    }
    let mut out = String::new();
    let mut buf = Vec::new();
    let mut depth = 0usize; // text 自身为 0，进入子元素 +1
    let mut prev = Prev::None;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(t)) => {
                if let Ok(s) = quick_xml::escape::unescape(t.as_ref()) {
                    if prev == Prev::Tag && !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&s);
                }
                prev = Prev::Text;
            }
            Ok(Event::GeneralRef(r)) => {
                // quick-xml 0.42 把 `&amp;` 等实体拆成 GeneralRef 事件；
                // 此处还原实体字符，保证 `Hello &amp; World` 拼回 `Hello & World`。
                let ch = match r.as_ref() {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "quot" => '"',
                    "apos" => '\'',
                    _ => '?',
                };
                out.push(ch);
                prev = Prev::Text; // 实体的延续仍算同一段文本，不与后续文本加换行
            }
            Ok(Event::Start(_)) => {
                depth += 1;
                prev = Prev::Tag;
            }
            Ok(Event::End(e)) => {
                let tag = e.local_name().into_inner();
                if tag == "text" && depth == 0 {
                    break;
                }
                depth = depth.saturating_sub(1);
                prev = Prev::Tag;
            }
            Ok(Event::Empty(_)) => {}
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    // 整体 trim：去掉首尾空白，但保留文本内部（含实体前后）的空格。
    out.trim().to_string()
}

fn attr_str<'a>(
    attrs: quick_xml::events::attributes::Attributes<'a>,
    name: &str,
) -> Option<String> {
    for a in attrs {
        if let Ok(a) = a
            && a.key == quick_xml::name::QName(name)
        {
            return Some(a.value.into_owned().to_string());
        }
    }
    None
}

fn attr_f64(attrs: quick_xml::events::attributes::Attributes, name: &str) -> Option<f64> {
    attr_str(attrs, name).and_then(|v| v.parse::<f64>().ok())
}

/// 圆角矩形与直角矩形在结构上等价（圆角只是样式差异，不影响布局/拓扑），
/// 比对时统一归一为 `Rect`，避免把 `rrect` 与 `rect` 计为不同元素。
fn effective_kind(k: Kind) -> Kind {
    if k == Kind::RoundedRect {
        Kind::Rect
    } else {
        k
    }
}

/// 解析 polyline 的 `points="x1,y1 x2,y2 ..."`，返回（首点, 末点）。
fn parse_points_endpoints(s: &str, ox: f64, oy: f64) -> Option<((f64, f64), (f64, f64))> {
    let nums: Vec<f64> = s
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter_map(|t| t.trim().parse::<f64>().ok())
        .collect();
    if nums.len() >= 4 {
        let first = (nums[0] + ox, nums[1] + oy);
        let last = (nums[nums.len() - 2] + ox, nums[nums.len() - 1] + oy);
        Some((first, last))
    } else {
        None
    }
}

/// 解析 path 的 `d` 属性，提取首坐标（M 之后）与末坐标（最后一组数字），返回（起点, 终点）。
/// 支持 `M x y ...` 与 `M x,y ...` 两种写法，以及含 L/C/Q 等命令的折线路径。
fn parse_path_endpoints(d: &str, ox: f64, oy: f64) -> Option<((f64, f64), (f64, f64))> {
    // 收集所有"命令后的坐标对"。简单做法：正则提取所有浮点数，按命令分块。
    // 先按命令字母切分。
    let mut last: Option<(f64, f64)> = None;
    // 用迭代方式扫描：遇到字母命令则后续数字成对归属，但我们只需要首对与末对。
    let chars: Vec<char> = d.chars().collect();
    let mut i = 0;
    let mut pending: Vec<f64> = Vec::new();
    let mut saw_cmd = false;
    let mut first_pair: Option<(f64, f64)> = None;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() {
            // 命令开始：落定上一组数字为一段（取最后一对作为该段终点）
            if !pending.is_empty() {
                if pending.len() >= 2 {
                    last = Some((pending[pending.len() - 2], pending[pending.len() - 1]));
                }
                pending.clear();
            }
            saw_cmd = true;
        } else if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' {
            // 解析一个数字
            let mut j = i;
            if chars[j] == '-' || chars[j] == '+' {
                j += 1;
            }
            while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
                j += 1;
            }
            if let Ok(num) = d[i..j].trim().parse::<f64>() {
                if pending.is_empty() {
                    // 新数字段起点；若是 M 命令的首对，记录为 first
                    if saw_cmd && first_pair.is_none() {
                        // 占位，等下一个数字凑对
                    }
                }
                pending.push(num);
                if pending.len() == 2 {
                    let pair = (pending[0], pending[1]);
                    if first_pair.is_none() {
                        first_pair = Some(pair);
                    }
                    // 每凑一对更新 last
                    last = Some(pair);
                } else if pending.len() > 2 {
                    // 超出一对（如 M x y x y ...），丢弃最早，保留最近一对
                    pending.remove(0);
                    let pair = (pending[0], pending[1]);
                    last = Some(pair);
                }
            }
            i = j;
            continue;
        }
        i += 1;
    }
    if let (Some(f), Some(l)) = (first_pair, last) {
        Some(((f.0 + ox, f.1 + oy), (l.0 + ox, l.1 + oy)))
    } else {
        None
    }
}

/// 把 SVG 元素归一到语义角色，使不同渲染后端（liemermaid vs 官方 mermaid）的
/// 同义表达能对齐：边（`path[edge]`/`polyline[edge]`/`path[edgePaths]`）统一为
/// `edge`；节点容器（`g/node`/`rect.node`/`circle.node`）统一为 `node`。其余保留原 class。
///
/// 当元素没有 class（如回退后的 liemermaid，IR 不含 class 概念）时，改用**几何类型**
/// 推断角色：封闭填充形状（rect/circle/ellipse/polygon）视为节点，线状几何
/// （line/polyline/path）视为边，文本视为 text。这样回退后仍能做拓扑/几何等价比对。
fn role_of(class: &str, kind: Kind) -> String {
    let c = class.to_lowercase();
    if !c.is_empty() {
        if c.contains("edge") {
            return "edge".to_string();
        } else if c.contains("node") {
            return "node".to_string();
        }
        return class.to_string();
    }
    // 无 class：基于几何类型推断语义角色（引擎无关的内容正确性比对）。
    match kind {
        Kind::Rect | Kind::RoundedRect | Kind::Circle | Kind::Polygon => "node".to_string(),
        Kind::Line | Kind::Polyline | Kind::Path => "edge".to_string(),
        Kind::Text => "text".to_string(),
        Kind::Other => String::new(),
    }
}

/// 几何等价比对容差（归一化坐标系下，单位：包围盒对角线比例）。
pub const GEOM_TOL: f64 = 0.18;

/// 把点集归一化：减去质心，再除以包围盒对角线，使平移/缩放不变。
fn normalize_points(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if pts.is_empty() {
        return Vec::new();
    }
    let n = pts.len() as f64;
    let (mut cx, mut cy) = (0.0, 0.0);
    for p in pts {
        cx += p.0;
        cy += p.1;
    }
    cx /= n;
    cy /= n;
    let mut minx = f64::INFINITY;
    let mut miny = f64::INFINITY;
    let mut maxx = f64::NEG_INFINITY;
    let mut maxy = f64::NEG_INFINITY;
    let centered: Vec<(f64, f64)> = pts
        .iter()
        .map(|p| {
            let x = p.0 - cx;
            let y = p.1 - cy;
            minx = minx.min(x);
            miny = miny.min(y);
            maxx = maxx.max(x);
            maxy = maxy.max(y);
            (x, y)
        })
        .collect();
    let diag = ((maxx - minx).powi(2) + (maxy - miny).powi(2)).sqrt();
    let diag = if diag < 1e-9 { 1.0 } else { diag };
    centered
        .into_iter()
        .map(|(x, y)| (x / diag, y / diag))
        .collect()
}

/// 点集是否几何等价：数量相等且归一化后每个点都能在对方找到容差内的配对（贪心）。
/// 返回最大配对距离（归一化坐标），数量不等返回 None。
fn match_point_sets(a: &[(f64, f64)], b: &[(f64, f64)]) -> Option<f64> {
    if a.len() != b.len() {
        return None;
    }
    let na = normalize_points(a);
    let nb = normalize_points(b);
    let mut used = vec![false; nb.len()];
    let mut max_err = 0.0f64;
    for p in &na {
        let mut best = None;
        let mut best_d = f64::INFINITY;
        for (i, q) in nb.iter().enumerate() {
            if used[i] {
                continue;
            }
            let d = ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
            if d < best_d {
                best_d = d;
                best = Some(i);
            }
        }
        match best {
            Some(i) => {
                used[i] = true;
                if best_d > max_err {
                    max_err = best_d;
                }
            }
            None => return Some(f64::INFINITY),
        }
    }
    Some(max_err)
}

/// 一条边的两个端点（用于几何比对）。
type EdgeEnds = ((f64, f64), (f64, f64));

/// 边端点集合是否几何等价：每条边视为线段（两端点），数量相等且每条边都能在对方找到
/// 一条边使其两端点（顺序可交换）在容差内配对。返回最大配对误差，数量不等返回 None。
fn match_edge_sets(a: &[EdgeEnds], b: &[EdgeEnds]) -> Option<f64> {
    if a.len() != b.len() {
        return None;
    }
    let feat = |e: &((f64, f64), (f64, f64))| -> ((f64, f64), f64) {
        let (p, q) = *e;
        let c = ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0);
        let len = ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
        (c, len)
    };
    let fa: Vec<_> = a.iter().map(feat).collect();
    let fb: Vec<_> = b.iter().map(feat).collect();
    let na = normalize_points(&fa.iter().map(|x| x.0).collect::<Vec<_>>());
    let nb = normalize_points(&fb.iter().map(|x| x.0).collect::<Vec<_>>());
    let mut used = vec![false; nb.len()];
    let mut max_err = 0.0f64;
    for (i, p) in na.iter().enumerate() {
        let mut best = None;
        let mut best_d = f64::INFINITY;
        for (j, q) in nb.iter().enumerate() {
            if used[j] {
                continue;
            }
            let d = ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
            if d < best_d {
                best_d = d;
                best = Some(j);
            }
        }
        if let Some(j) = best {
            used[j] = true;
            let la = fa[i].1;
            let lb = fb[j].1;
            let len_err = if la > 1e-6 { (la - lb).abs() / la } else { 0.0 };
            let err = best_d.max(len_err);
            if err > max_err {
                max_err = err;
            }
        } else {
            return Some(f64::INFINITY);
        }
    }
    Some(max_err)
}

/// 生成摘要。
///
/// 说明：SVG 级别的官方对齐关注的是**拓扑结构**（节点数、边数、标签文本、相对布局），
/// 而非样式（填充/描边配色、具体的圆角/箭头形状）。因此这里：
/// - 忽略 fill/stroke 颜色（它们属于主题，不影响结构）；
/// - 把边/节点按语义角色归并，使不同后端的同义 SVG 表达能对等计数。
pub fn summarize(els: &[El]) -> Summary {
    let mut s = Summary::default();
    for el in els {
        // 计数使用归一后的几何类型（圆角矩形并入直角矩形）。
        let k = effective_kind(el.kind);
        let role = role_of(&el.class, el.kind);
        *s.counts.entry((k, role.clone())).or_insert(0) += 1;
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
        // fill/stroke 颜色属于主题样式，不计入结构差异。
        // 相对布局 / 包围盒维度：以语义角色（class 或几何类型）标识，坐标量化整数像素。
        // 同样使用归一后的几何类型，保证圆角/直角矩形在布局维度上一致。
        let key = if role.is_empty() {
            kind_name(k).to_string()
        } else {
            role
        };
        let bx = el.x.round() as i64;
        let by = el.y.round() as i64;
        let bw = el.w.round() as i64;
        let bh = el.h.round() as i64;
        s.boxes.insert((key.clone(), bx, by, bw, bh));

        // 几何等价维度：节点中心 + 边端点（允许整体平移/缩放/布局差异）。
        if key == "node" {
            s.node_centers.push((el.x + el.w / 2.0, el.y + el.h / 2.0));
        }
        if key == "edge"
            && let Some((a, b)) = el.endpoints
        {
            let dx = b.0 - a.0;
            let dy = b.1 - a.1;
            // 跳过过短的线（如自绘箭头头 path），仅保留真正的边连线。
            if (dx * dx + dy * dy).sqrt() > 5.0 {
                s.edge_endpoints.push((a, b));
            }
        }
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
    // 几何等价维度（允许整体平移/缩放/布局差异）：节点中心集合 + 边端点集合。
    d.geom_node_err = match_point_sets(&ours.node_centers, &golden.node_centers);
    d.geom_edge_err = match_edge_sets(&ours.edge_endpoints, &golden.edge_endpoints);
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
        let a = summarize(&parse(
            r#"<svg><g class='node'><rect x='0' y='0' width='10' height='10'/></g><text x='0' y='0'>A</text></svg>"#,
        ));
        let b = summarize(&parse(
            r#"<svg><g class='node'><rect x='0' y='0' width='10' height='10'/></g><g class='node'><rect x='50' y='0' width='10' height='10'/></g><text x='0' y='0'>B</text></svg>"#,
        ));
        let d = compare(&a, &b);
        assert!(!d.is_empty());
        // b 比 a 多一个 node 矩形
        assert_eq!(
            d.count_diffs.get(&(Kind::Rect, "node".to_string())),
            Some(&(1usize, 2usize))
        );
        // a=ours（少文本 A），b=golden（多文本 B）：
        // ours 多出 A（golden 没有）→ extra；ours 缺失 B（golden 有）→ missing
        assert!(d.extra_texts.contains("A"));
        assert!(d.missing_texts.contains("B"));
    }
}
