use crate::{
    ast::Diagram,
    diagram_builder::{
        types::OutputConfig,
        flowchart::build_flowchart_elements,
        pie::build_pie_elements,
    },
    error::{DiagramError, DiagramResult},
    visual::VisualElement,
};

/// 构建 Mermaid Diagram 的视觉元素管线
///
/// # 管线流程
///
/// 1. 分发：根据 Diagram 枚举类型分发到对应的 builder
/// 2. 布局：计算各元素的尺寸与位置
/// 3. 构建：产出 Vec<VisualElement> 供渲染器消费
pub fn build_diagram(diagram: &Diagram) -> DiagramResult<Vec<VisualElement>> {
    build_diagram_with_config(diagram, &OutputConfig::default())
}

pub fn build_diagram_with_config(
    diagram: &Diagram,
    config: &OutputConfig,
) -> DiagramResult<Vec<VisualElement>> {
    match diagram {
        Diagram::Pie(pie) => build_pie_elements(pie, config),
        Diagram::Flowchart(flowchart) => build_flowchart_elements(flowchart, config),
        Diagram::Sequence(_) => Err(DiagramError::UnsupportedType(
            "sequence diagram builder not yet implemented".into(),
        )),
        Diagram::Class(_) => Err(DiagramError::UnsupportedType(
            "class diagram builder not yet implemented".into(),
        )),
        Diagram::State(_) => Err(DiagramError::UnsupportedType(
            "state diagram builder not yet implemented".into(),
        )),
        Diagram::Er(_) => Err(DiagramError::UnsupportedType(
            "ER diagram builder not yet implemented".into(),
        )),
        Diagram::Timeline(_) => Err(DiagramError::UnsupportedType(
            "timeline diagram builder not yet implemented".into(),
        )),
        Diagram::GitGraph(_) => Err(DiagramError::UnsupportedType(
            "gitgraph diagram builder not yet implemented".into(),
        )),
    }
}

pub mod flowchart;
pub mod layout;
pub mod pie;
pub mod types;