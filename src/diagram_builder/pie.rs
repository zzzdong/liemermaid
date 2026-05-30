use vello_cpu::kurbo::Point;

use crate::{
    ast::PieDiagram,
    diagram_builder::types::{OutputConfig, PALETTE},
    error::{DiagramError, DiagramResult},
    text::create_text_layout,
    visual::{
        Color, FillStrokeStyle, TextAlign, TextBaseline, TextStyle,
        VisualElement, Z_LABEL, Z_SERIES, Z_TITLE,
    },
};

const PIE_MARGIN: f64 = 60.0;
const PIE_TITLE_SIZE: f64 = 24.0;
const PIE_LABEL_SIZE: f64 = 13.0;
const PIE_LABEL_OFFSET: f64 = 20.0;

pub fn build_pie_elements(
    pie: &PieDiagram,
    config: &OutputConfig,
) -> DiagramResult<Vec<VisualElement>> {
    let mut elements = Vec::new();

    // 解析数值
    let mut data_values: Vec<(String, f64)> = Vec::new();
    for d in &pie.data {
        let value: f64 = d
            .value
            .parse()
            .map_err(|_| DiagramError::LayoutError(format!("invalid pie value: {}", d.value)))?;
        data_values.push((d.label.clone(), value));
    }

    let total: f64 = data_values.iter().map(|(_, v)| v).sum();
    if total <= 0.0 {
        return Err(DiagramError::LayoutError("pie total must be > 0".into()));
    }

    // 计算饼图区域
    let cx = config.width / 2.0;
    let cy = if pie.title.is_some() {
        config.height / 2.0 + 20.0
    } else {
        config.height / 2.0
    };
    let radius = (config.width.min(config.height) / 2.0 - PIE_MARGIN).max(50.0);

    // 标题
    if let Some(title) = &pie.title {
        let title_style = TextStyle {
            font_size: PIE_TITLE_SIZE,
            align: TextAlign::Center,
            vertical_align: TextBaseline::Top,
            ..Default::default()
        };
        let layout = create_text_layout(title, &title_style, Some(config.width - PIE_MARGIN));
        let title_x = config.width / 2.0;
        let title_y = PIE_MARGIN / 2.0;

        elements.push(VisualElement::TextRun {
            text: title.clone(),
            position: Point::new(title_x, title_y),
            style: title_style,
            rotation: 0.0,
            max_width: Some(config.width - PIE_MARGIN),
            layout: Some(layout),
            z_index: Z_TITLE,
        });
    }

    // 绘制各扇区
    let mut start_angle = -std::f64::consts::FRAC_PI_2; // 从12点钟方向开始
    for (idx, (label, value)) in data_values.iter().enumerate() {
        let slice_angle = 2.0 * std::f64::consts::PI * value / total;
        let end_angle = start_angle + slice_angle;
        let color = PALETTE[idx % PALETTE.len()];

        // 扇区路径
        let (sx, sy) = (start_angle.cos(), start_angle.sin());

        let mut path = vello_cpu::kurbo::BezPath::new();
        path.move_to(Point::new(cx, cy));
        path.line_to(Point::new(cx + radius * sx, cy + radius * sy));
        // 使用多段直线近似扇形外弧（避免 kurbo arc_to 版本兼容问题）
        let arc_segments = 20;
        for i in 1..=arc_segments {
            let t = i as f64 / arc_segments as f64;
            let angle = start_angle + t * slice_angle;
            path.line_to(Point::new(cx + radius * angle.cos(), cy + radius * angle.sin()));
        }
        path.close_path();

        // 饼图扇区（带描边分割）
        let style = FillStrokeStyle::new()
            .with_fill(color)
            .with_stroke(Color::new(255, 255, 255), 2.0);

        elements.push(VisualElement::Path {
            path,
            style,
            z_index: Z_SERIES,
        });

        // 标签：放在扇形外缘中间位置
        let mid_angle = start_angle + slice_angle / 2.0;
        let label_radius = radius + PIE_LABEL_OFFSET;
        let lx = cx + label_radius * mid_angle.cos();
        let ly = cy + label_radius * mid_angle.sin();

        let pct = format!("{:.1}%", value / total * 100.0);
        let display_text = if pie.show_data {
            format!("{} ({} {:.0})", label, pct, value)
        } else {
            format!("{} ({})", label, pct)
        };

        let label_style = TextStyle {
            font_size: PIE_LABEL_SIZE,
            align: TextAlign::Center,
            vertical_align: TextBaseline::Middle,
            ..Default::default()
        };
        let label_layout = create_text_layout(&display_text, &label_style, Some(200.0));

        elements.push(VisualElement::TextRun {
            text: display_text,
            position: Point::new(lx, ly),
            style: label_style,
            rotation: 0.0,
            max_width: Some(200.0),
            layout: Some(label_layout),
            z_index: Z_LABEL,
        });

        start_angle = end_angle;
    }

    Ok(elements)
}