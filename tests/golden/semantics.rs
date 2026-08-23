//! 语义层提取：从任意 mermaid 风格 SVG 抽取结构化语义，用于跨引擎（官方 mermaid-cli
//! vs liemermaid/lievisual）的正确性比对。
//!
//! 设计动机：坐标、元素数量、颜色等是**引擎相关**的（官方用 `<path>`+marker 画边，
//! liemermaid 用 `<polyline>`；官方文本在 `foreignObject>div>span`，liemermaid 在裸
//! `<text>`）。这些不能作为"对不对"的判据。真正可比的是**语义**：
//!
//! 1. **文本集合**——同一张图，两边出现的文字必须一致（节点名/actor/消息/字段/切片标签）。
//! 2. **节点标签集合**——承载语义的节点（node/actor/class/er/state/timeline/commit/slice）
//!    的文本集合必须一致。
//! 3. **语义类型计数**——归一化后的语义标签（node/edge/actor/message/...）数量分布，
//!    用于"结构形态"软比对（引擎差异允许偏差，但缺失某类语义 = 结构错误）。
//!
//! 归一化：把两家不同的 `class` 体系映射到统一的语义标签。官方 class 丰富
//!（`node`、`flowchart-v2`、`edge-thickness-normal`、`actor`、`messageText`、
//! `classGroup`、`er`、`stateGroup`、`timeline-node`、`commit` 等）；liemermaid 当前
//! 仅输出 `node`/`edge`/`label` 等少量 class——这正是"改 lievisual 让结构一致"的目标。

use quick_xml::events::Event;
use std::collections::{BTreeMap, BTreeSet};

/// 归一化语义标签。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sem {
    Node,      // 通用节点（flowchart 节点、class/er/state 实体、timeline 节点、git commit）
    Edge,      // 边 / 连接
    Actor,     // sequence 参与者
    Message,   // sequence 消息文本
    Note,      // sequence/class/er 标注
    Slice,     // pie 扇区
    Label,     // 节点/边上的纯标签文本容器
    Other,
}

impl Sem {
    fn name(self) -> &'static str {
        match self {
            Sem::Node => "node",
            Sem::Edge => "edge",
            Sem::Actor => "actor",
            Sem::Message => "message",
            Sem::Note => "note",
            Sem::Slice => "slice",
            Sem::Label => "label",
            Sem::Other => "other",
        }
    }
}

/// 官方 mermaid SVG 的 class → 语义标签映射（多值 class 命中任一即归类）。
fn official_class_sem(cls: &str) -> Option<Sem> {
    let c = cls.trim();
    if c.is_empty() {
        return None;
    }
    // 多值 class：拆空格，任一命中即返回
    for part in c.split_whitespace() {
        let s = match part {
            "node" => Some(Sem::Node),
            "root" => Some(Sem::Node),
            "actor" => Some(Sem::Actor),
            "messageText" | "messageLine" | "message" => Some(Sem::Message),
            "note" | "noteText" | "noteText0" => Some(Sem::Note),
            "classGroup" | "classLabel" | "entityBox" | "entity" => Some(Sem::Node),
            "er" | "erLabel" | "relationship" => Some(Sem::Node),
            "stateGroup" | "state-title" | "state" => Some(Sem::Node),
            "stateLabel" | "transition" | "transitionLabel" => Some(Sem::Edge),
            "edgeLabel" | "edge" | "edge-thickness-normal" | "edge-pattern-solid"
            | "edge-thickness-thick" | "edge-pattern-dashed" => Some(Sem::Edge),
            "timeline-node" | "commit" | "commit-id" | "commit-msg" => Some(Sem::Node),
            "section" => Some(Sem::Node),
            "label" | "nodeLabel" | "flowchart-label" => Some(Sem::Label),
            _ => None,
        };
        if let Some(s) = s {
            return Some(s);
        }
    }
    None
}

/// liemermaid/lievisual SVG 的 class → 语义标签映射（当前仅少量 class，
/// 改造后应逐步贴近官方语义）。
fn liemermaid_class_sem(cls: &str) -> Option<Sem> {
    let c = cls.trim();
    if c.is_empty() {
        return None;
    }
    for part in c.split_whitespace() {
        let s = match part {
            "node" => Some(Sem::Node),
            "edge" => Some(Sem::Edge),
            "actor" => Some(Sem::Actor),
            "message" | "messageText" => Some(Sem::Message),
            "note" => Some(Sem::Note),
            "label" => Some(Sem::Label),
            "slice" => Some(Sem::Slice),
            "commit" => Some(Sem::Node),
            "state" => Some(Sem::Node),
            "transition" | "edgeLabel" => Some(Sem::Edge),
            _ => None,
        };
        if let Some(s) = s {
            return Some(s);
        }
    }
    None
}

/// 抽取出的语义。
#[derive(Debug, Clone, Default)]
pub struct DiagramSemantics {
    /// 归一语义标签计数。
    pub types: BTreeMap<Sem, usize>,
    /// 全部可见文本（去重）。
    pub texts: BTreeSet<String>,
    /// 节点类元素的文本（去重）——承载语义身份的标签。
    pub node_labels: BTreeSet<String>,
}

impl DiagramSemantics {
    #[allow(dead_code)]
    pub fn describe(&self) -> String {
        let mut s = String::new();
        for (k, v) in &self.types {
            s.push_str(&format!("    {}: {}\n", k.name(), v));
        }
        s
    }
}

/// 从 SVG 抽取语义。
///
/// `official`: true 用官方 class 映射，false 用 liemermaid 映射。
pub fn extract(svg: &str, official: bool) -> DiagramSemantics {
    let mut sem = DiagramSemantics::default();
    let mut reader = quick_xml::Reader::from_str(svg);
    // group 栈：记录最近 class（用于给无 class 的子元素继承语义——官方节点文本在
    // 文本载体层级：官方用 foreignObject>div>span，liemermaid 用裸 <text>/<tspan>。
    // 统一收集所有文本节点到 `texts`；节点身份标签（node/actor）按栈顶 class 上下文判定。
    // 用 Vec<Option<String>> 记录每个容器 start 是否 push 了 class，确保 End 精确配对 pop。
    let mut class_stack: Vec<Option<String>> = Vec::new();
    // <style> 内的 CSS 文本是噪声，不是图标签，需跳过其文本收集。
    let mut style_depth: usize = 0;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let lname = e.local_name();
                let tag = std::str::from_utf8(lname.as_ref()).unwrap_or("");
                if tag == "style" {
                    style_depth += 1;
                    class_stack.push(None);
                } else if tag == "g" || tag == "div" || tag == "span" || tag == "foreignObject" {
                    let cls = attr_str(e.attributes(), "class").unwrap_or_default();
                    if cls.is_empty() {
                        class_stack.push(None);
                    } else {
                        class_stack.push(Some(cls.clone()));
                        // 容器自身携带语义 class（如官方 <g class="node">、<g class="edge-...">）
                        // 直接计数，避免依赖内部元素重复计。
                        let sem_of = if official {
                            official_class_sem(&cls)
                        } else {
                            liemermaid_class_sem(&cls)
                        };
                        if let Some(s) = sem_of {
                            *sem.types.entry(s).or_insert(0) += 1;
                        } else if !cls.is_empty() {
                            *sem.types.entry(Sem::Other).or_insert(0) += 1;
                        }
                    }
                } else {
                    // 非容器标签（如 text/tspan/path/rect）不压栈
                    class_stack.push(None);
                }
            }
            Ok(Event::Text(t)) => {
                if style_depth > 0 {
                    // 跳过 <style> 内的 CSS 噪声
                } else if let Ok(s) = t.unescape() {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        sem.texts.insert(trimmed.to_string());
                        // 节点身份判定：看栈顶最近的 class（可能是 None）
                        let inherited = class_stack
                            .iter()
                            .rev()
                            .filter_map(|c| c.as_ref())
                            .next()
                            .cloned()
                            .unwrap_or_default();
                        let sem_of = if official {
                            official_class_sem(&inherited)
                        } else {
                            liemermaid_class_sem(&inherited)
                        };
                        if matches!(sem_of, Some(Sem::Node) | Some(Sem::Actor)) {
                            sem.node_labels.insert(trimmed.to_string());
                        }
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let lname = e.local_name();
                let _ = lname;
                let cls = attr_str(e.attributes(), "class").unwrap_or_default();
                // 空元素（如 <rect class="node">、<path class="edge">）仅按**自身** class 计数；
                // 不继承父容器 class（容器已在 Start 时计过，避免重复）。
                let direct = if official {
                    official_class_sem(&cls)
                } else {
                    liemermaid_class_sem(&cls)
                };
                if let Some(s) = direct {
                    *sem.types.entry(s).or_insert(0) += 1;
                } else if !cls.is_empty() {
                    // 未知但有 class 的元素，归 Other 以便暴露结构差异
                    *sem.types.entry(Sem::Other).or_insert(0) += 1;
                }
            }
            Ok(Event::End(e)) => {
                if !class_stack.is_empty() {
                    class_stack.pop();
                }
                let lname = e.local_name();
                let tag = std::str::from_utf8(lname.as_ref()).unwrap_or("");
                if tag == "style" && style_depth > 0 {
                    style_depth -= 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    sem
}

fn attr_str<'a>(
    attrs: quick_xml::events::attributes::Attributes<'a>,
    name: &str,
) -> Option<String> {
    for a in attrs {
        if let Ok(a) = a {
            if a.key.as_ref() == name.as_bytes() {
                return a.unescape_value().ok().map(|v| v.to_string());
            }
        }
    }
    None
}

/// 语义差异（用于测试断言与诊断输出）。
#[derive(Debug, Clone, Default)]
pub struct SemDiff {
    pub missing_texts: BTreeSet<String>,
    pub extra_texts: BTreeSet<String>,
    pub missing_node_labels: BTreeSet<String>,
    pub extra_node_labels: BTreeSet<String>,
    pub type_diffs: BTreeMap<Sem, (usize, usize)>,
}

impl SemDiff {
    pub fn is_empty(&self) -> bool {
        self.missing_texts.is_empty()
            && self.extra_texts.is_empty()
            && self.missing_node_labels.is_empty()
            && self.extra_node_labels.is_empty()
            && self.type_diffs.is_empty()
    }

    pub fn describe(&self) -> String {
        let mut s = String::new();
        for t in &self.missing_texts {
            s.push_str(&format!("  missing text: {t:?}\n"));
        }
        for t in &self.extra_texts {
            s.push_str(&format!("  extra text:   {t:?}\n"));
        }
        for t in &self.missing_node_labels {
            s.push_str(&format!("  missing node-label: {t:?}\n"));
        }
        for t in &self.extra_node_labels {
            s.push_str(&format!("  extra node-label:   {t:?}\n"));
        }
        for (k, (a, b)) in &self.type_diffs {
            s.push_str(&format!("  type {}: ours={a} golden={b}\n", k.name()));
        }
        s
    }
}

/// 比较 semantics（ours = liemermaid 当前，golden = 官方）。
/// 文本/节点标签要求**集合相等**（强）；类型计数记录偏差（弱，由调用方决定 fail/warn）。
pub fn compare(ours: &DiagramSemantics, golden: &DiagramSemantics) -> SemDiff {
    let mut d = SemDiff::default();
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
    for t in &golden.node_labels {
        if !ours.node_labels.contains(t) {
            d.missing_node_labels.insert(t.clone());
        }
    }
    for t in &ours.node_labels {
        if !golden.node_labels.contains(t) {
            d.extra_node_labels.insert(t.clone());
        }
    }
    let mut keys: BTreeSet<Sem> = BTreeSet::new();
    keys.extend(ours.types.keys().cloned());
    keys.extend(golden.types.keys().cloned());
    for k in keys {
        let a = *ours.types.get(&k).unwrap_or(&0);
        let b = *golden.types.get(&k).unwrap_or(&0);
        if a != b {
            d.type_diffs.insert(k, (a, b));
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_flowchart_node_and_edge() {
        let svg = r#"<svg><g class="node default"><foreignObject><div><span>Start</span></div></foreignObject></g><g class="edge-thickness-normal edge-pattern-solid"><path d="M0 0"/></g></svg>"#;
        let s = extract(svg, true);
        assert_eq!(s.types.get(&Sem::Node), Some(&1));
        assert_eq!(s.types.get(&Sem::Edge), Some(&1));
        assert!(s.texts.contains("Start"));
        assert!(s.node_labels.contains("Start"));
    }

    #[test]
    fn liemermaid_flowchart_node_and_edge() {
        let svg = r#"<svg><g class="node"><rect/><text>End</text></g><g class="edge"><polyline/></g></svg>"#;
        let s = extract(svg, false);
        assert_eq!(s.types.get(&Sem::Node), Some(&1));
        assert_eq!(s.types.get(&Sem::Edge), Some(&1));
        assert!(s.texts.contains("End"));
        assert!(s.node_labels.contains("End"));
    }

    #[test]
    fn compare_text_equality() {
        let a = extract(r#"<svg><g class="node"><text>A</text></g><text>B</text></svg>"#, false);
        let b = extract(r#"<svg><g class="node"><text>A</text></g><text>C</text></svg>"#, false);
        let d = compare(&a, &b);
        assert!(d.missing_texts.contains("C"));
        assert!(d.extra_texts.contains("B"));
    }
}
