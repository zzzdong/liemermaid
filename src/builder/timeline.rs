use lievisual::geometry::Point;

use crate::{
    ast::TimelineDiagram,
    builder::{layout::types::LayoutEngine, types::OutputConfig},
    error::DiagramResult,
    vir::{self, SceneNode, TextAlign, TextBaseline, Z_AXIS, Z_LABEL, Z_SERIES, Z_TITLE, theme},
};
use lievisual::text::{RichSpan, compute_text_offset, layout_text};

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
    fn layout(&self, config: &OutputConfig) -> DiagramResult<Vec<SceneNode>> {
        Ok(build_timeline_elements(self.timeline, config))
    }
}

pub fn build_timeline_elements(
    timeline: &TimelineDiagram,
    config: &OutputConfig,
) -> Vec<SceneNode> {
    let mut elements = Vec::new();

    if timeline.sections.is_empty() {
        return elements;
    }

    // Title
    let mut cur_y = 30.0;
    if let Some(title) = &timeline.title {
        let ts = vir::text_style(
            theme::timeline::TITLE,
            TITLE_SIZE,
            theme::FONT_FAMILY.to_string(),
            TextAlign::Center,
            TextBaseline::Top,
        );
        let layout = layout_text(
            &[RichSpan::new(title.to_string(), ts.clone())],
            Some(config.width - 80.0),
        );
        let (x_off, y_off) = compute_text_offset(&layout, TextAlign::Center, TextBaseline::Top);
        elements.push(vir::text_node(
            title.clone(),
            Point::new(config.width / 2.0 + x_off, cur_y + y_off),
            ts.clone()
                .with_align(TextAlign::Left)
                .with_baseline(TextBaseline::Top),
            0.0,
            Some(config.width - 80.0),
            Z_TITLE,
        ));
        cur_y += 50.0;
    }

    // Timeline horizontal line
    let line_y = cur_y + 10.0;
    let left_margin = 40.0;
    let right_margin = config.width - 40.0;
    elements.push(vir::line_node(
        Point::new(left_margin, line_y),
        Point::new(right_margin, line_y),
        vir::stroke(theme::timeline::LINE, 2.5),
        Z_AXIS,
    ));

    // Sections and events
    let section_spacing = (right_margin - left_margin) / timeline.sections.len() as f64;
    cur_y = line_y + 10.0;

    for (i, section) in timeline.sections.iter().enumerate() {
        let cx = left_margin + section_spacing * (i as f64 + 0.5);

        // Dot on timeline
        elements.push(vir::circle_node(
            Point::new(cx, line_y),
            5.0,
            vir::fs_both(theme::timeline::LINE, theme::timeline::LINE, 2.0),
            Z_SERIES,
        ).with_class("node"));

        // Section name
        let ts = vir::text_style(
            theme::timeline::TEXT,
            SECTION_SIZE,
            theme::FONT_FAMILY.to_string(),
            TextAlign::Center,
            TextBaseline::Top,
        );
        let layout = layout_text(
            &[RichSpan::new(section.name.to_string(), ts.clone())],
            Some(section_spacing - 10.0),
        );
        let line_count = layout.lines.len().max(1);
        let estimated_h = line_count as f64 * 20.0;

        elements.push(vir::text_node(
            section.name.clone(),
            Point::new(cx - (section_spacing - 10.0) / 2.0, cur_y),
            ts.clone()
                .with_align(TextAlign::Left)
                .with_baseline(TextBaseline::Top),
            0.0,
            Some(section_spacing - 10.0),
            Z_LABEL,
        ));

        let mut event_y = cur_y + estimated_h + 10.0;

        // Events
        for event in &section.events {
            // Event dot
            elements.push(vir::circle_node(
                Point::new(cx, event_y + 4.0),
                3.0,
                vir::fs_fill(theme::timeline::LINE),
                Z_SERIES,
            ).with_class("node"));

            // Vertical connector line
            elements.push(vir::line_node(
                Point::new(cx, cur_y + estimated_h),
                Point::new(cx, event_y + 4.0),
                vir::stroke(theme::timeline::LINE, 1.0),
                Z_AXIS,
            ));

            let ets = vir::text_style(
                theme::timeline::TEXT,
                EVENT_SIZE,
                theme::FONT_FAMILY.to_string(),
                TextAlign::Left,
                TextBaseline::Top,
            );
            let _el = layout_text(
                &[RichSpan::new(event.to_string(), ets.clone())],
                Some(section_spacing - 25.0),
            );
            elements.push(vir::text_node(
                event.clone(),
                Point::new(cx + 10.0, event_y),
                ets.clone()
                    .with_align(TextAlign::Left)
                    .with_baseline(TextBaseline::Top),
                0.0,
                Some(section_spacing - 25.0),
                Z_LABEL,
            ));

            event_y += 22.0;
        }

        // Set cur_y for next section reference
        cur_y = cur_y.max(event_y);
    }

    elements
}
