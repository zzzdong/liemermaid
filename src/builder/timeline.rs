use vello_cpu::kurbo::Point;

use crate::{
    ast::TimelineDiagram,
    builder::{
        layout::types::LayoutEngine,
        types::OutputConfig,
    },
    error::DiagramResult,
    text::{compute_text_offset, create_text_layout},
    visual::{
        theme,
        FillStrokeStyle, StrokeStyle, TextAlign, TextBaseline, TextStyle,
        VisualElement, Z_AXIS, Z_LABEL, Z_SERIES, Z_TITLE,
    },
};

const TITLE_SIZE: f64 = 22.0;
const SECTION_SIZE: f64 = 14.0;
const EVENT_SIZE: f64 = 12.0;

pub struct TimelineEngine<'a> {
    timeline: &'a TimelineDiagram,
}

impl<'a> TimelineEngine<'a> {
    pub fn new(timeline: &'a TimelineDiagram) -> Self {
        Self { timeline }
    }
}

impl<'a> LayoutEngine for TimelineEngine<'a> {
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<VisualElement>> {
        Ok(build_timeline_elements(self.timeline, config))
    }
}

pub fn build_timeline_elements(
    timeline: &TimelineDiagram,
    config: &OutputConfig,
) -> Vec<VisualElement> {
    let mut elements = Vec::new();

    if timeline.sections.is_empty() {
        return elements;
    }

    // Title
    let mut cur_y = 30.0;
    if let Some(title) = &timeline.title {
        let ts = TextStyle {
            font_size: TITLE_SIZE,
            font_family: theme::FONT_FAMILY.to_string(),
            align: TextAlign::Center,
            vertical_align: TextBaseline::Top,
            color: theme::timeline::TITLE,
            ..Default::default()
        };
        let layout = create_text_layout(title, &ts, Some(config.width - 80.0));
        let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Top);
        elements.push(VisualElement::TextRun {
            text: title.clone(),
            position: Point::new(config.width / 2.0 + x_off, cur_y + y_off),
            style: TextStyle { align: TextAlign::Left, vertical_align: TextBaseline::Top, ..ts },
            rotation: 0.0,
            max_width: Some(config.width - 80.0),
            layout: Some(layout),
            z_index: Z_TITLE,
        });
        cur_y += 50.0;
    }

    // Timeline horizontal line
    let line_y = cur_y + 10.0;
    let left_margin = 40.0;
    let right_margin = config.width - 40.0;
    elements.push(VisualElement::Line {
        start: Point::new(left_margin, line_y),
        end: Point::new(right_margin, line_y),
        style: StrokeStyle { color: theme::timeline::LINE, width: 2.5 },
        z_index: Z_AXIS,
    });

    // Sections and events
    let section_spacing = (right_margin - left_margin) / timeline.sections.len() as f64;
    cur_y = line_y + 10.0;

    for (i, section) in timeline.sections.iter().enumerate() {
        let cx = left_margin + section_spacing * (i as f64 + 0.5);

        // Dot on timeline
        elements.push(VisualElement::Circle {
            center: Point::new(cx, line_y),
            radius: 5.0,
            style: FillStrokeStyle::new()
                .with_fill(theme::timeline::LINE)
                .with_stroke(theme::timeline::LINE, 2.0),
            z_index: Z_SERIES,
        });

        // Section name
        let ts = TextStyle {
            font_size: SECTION_SIZE,
            font_family: theme::FONT_FAMILY.to_string(),
            align: TextAlign::Center,
            vertical_align: TextBaseline::Top,
            color: theme::timeline::TEXT,
            ..Default::default()
        };
        let layout = create_text_layout(&section.name, &ts, Some(section_spacing - 10.0));
        let line_count = layout.lines().count().max(1);
        let estimated_h = line_count as f64 * 20.0;

        elements.push(VisualElement::TextRun {
            text: section.name.clone(),
            position: Point::new(cx - (section_spacing - 10.0) / 2.0, cur_y),
            style: TextStyle { align: TextAlign::Left, vertical_align: TextBaseline::Top, ..ts },
            rotation: 0.0,
            max_width: Some(section_spacing - 10.0),
            layout: Some(layout),
            z_index: Z_LABEL,
        });

        let mut event_y = cur_y + estimated_h + 10.0;

        // Events
        for event in &section.events {
            // Event dot
            elements.push(VisualElement::Circle {
                center: Point::new(cx, event_y + 4.0),
                radius: 3.0,
                style: FillStrokeStyle::new().with_fill(theme::timeline::LINE),
                z_index: Z_SERIES,
            });

            // Vertical connector line
            elements.push(VisualElement::Line {
                start: Point::new(cx, cur_y + estimated_h),
                end: Point::new(cx, event_y + 4.0),
                style: StrokeStyle { color: theme::timeline::LINE, width: 1.0 },
                z_index: Z_AXIS,
            });

            let ets = TextStyle {
                font_size: EVENT_SIZE,
                font_family: theme::FONT_FAMILY.to_string(),
                align: TextAlign::Left,
                vertical_align: TextBaseline::Top,
                color: theme::timeline::TEXT,
                ..Default::default()
            };
            let el = create_text_layout(event, &ets, Some(section_spacing - 25.0));
            elements.push(VisualElement::TextRun {
                text: event.clone(),
                position: Point::new(cx + 10.0, event_y),
                style: TextStyle { align: TextAlign::Left, vertical_align: TextBaseline::Top, ..ets },
                rotation: 0.0,
                max_width: Some(section_spacing - 25.0),
                layout: Some(el),
                z_index: Z_LABEL,
            });

            event_y += 22.0;
        }

        // Set cur_y for next section reference
        cur_y = cur_y.max(event_y);
    }

    elements
}
