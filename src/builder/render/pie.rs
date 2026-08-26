//! Pie 渲染器：复用既有几何算法，按新管线统一入口绘制。

use lievisual::geometry::Point;
use lievisual::text::{RichSpan, layout_text};

use crate::{
    ast::PieDiagram,
    builder::{theme, types::OutputConfig},
    error::{DiagramError, DiagramResult},
    vir::{self, Color, SceneNode, TextAlign, TextBaseline, Z_LABEL, Z_SERIES, Z_TITLE},
};

const PIE_MARGIN: f64 = 60.0;
const PIE_TITLE_SIZE: f64 = 24.0;
const PIE_LABEL_SIZE: f64 = 13.0;
const PIE_LABEL_OFFSET: f64 = 20.0;

/// 把饼图 AST 渲染为视觉元素（复用原 `build_pie_elements` 的几何算法）。
pub fn render_pie(pie: &PieDiagram, config: &OutputConfig) -> DiagramResult<Vec<SceneNode>> {
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
        let title_style = vir::text_style(
            Color::BLACK,
            PIE_TITLE_SIZE,
            String::new(),
            TextAlign::Center,
            TextBaseline::Top,
        );
        let _layout = layout_text(
            &[RichSpan::new(title.to_string(), title_style.clone())],
            Some(config.width - PIE_MARGIN),
        );
        let title_x = config.width / 2.0;
        let title_y = PIE_MARGIN / 2.0;

        elements.push(vir::text_node(
            title.clone(),
            Point::new(title_x, title_y),
            title_style,
            0.0,
            Some(config.width - PIE_MARGIN),
            Z_TITLE,
        ));
    }

    // 绘制各扇区
    let mut start_angle = -std::f64::consts::FRAC_PI_2; // 从12点钟方向开始
    for (idx, (label, value)) in data_values.iter().enumerate() {
        let slice_angle = 2.0 * std::f64::consts::PI * value / total;
        let end_angle = start_angle + slice_angle;
        let color = theme::pie::COLORS[idx % theme::pie::COLORS.len()];

        // 扇区路径
        let (sx, sy) = (start_angle.cos(), start_angle.sin());

        let mut path = lievisual::geometry::BezPath::new();
        path.move_to(Point::new(cx, cy));
        path.line_to(Point::new(cx + radius * sx, cy + radius * sy));

        let arc_segments = 20;
        for i in 1..=arc_segments {
            let t = i as f64 / arc_segments as f64;
            let angle = start_angle + t * slice_angle;
            path.line_to(Point::new(
                cx + radius * angle.cos(),
                cy + radius * angle.sin(),
            ));
        }
        path.close_path();

        // 饼图扇区（带描边分割）
        let style = vir::fs_both(color, Color::rgb(255, 255, 255), 2.0);

        elements.push(vir::path_node(path, style, Z_SERIES));

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

        let label_style = vir::text_style(
            Color::BLACK,
            PIE_LABEL_SIZE,
            String::new(),
            TextAlign::Center,
            TextBaseline::Middle,
        );
        let _label_layout = layout_text(
            &[RichSpan::new(display_text.to_string(), label_style.clone())],
            Some(200.0),
        );

        elements.push(vir::text_node(
            display_text,
            Point::new(lx, ly),
            label_style,
            0.0,
            Some(200.0),
            Z_LABEL,
        ));

        start_angle = end_angle;
    }

    Ok(elements)
}
