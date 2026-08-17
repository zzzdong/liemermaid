use std::{cell::RefCell, sync::Arc};

use parley::{
    Alignment, AlignmentOptions, FontContext, LayoutContext,
    style::{FontFamily, StyleProperty},
};

use crate::error::DiagramError;
use lievisual::geometry::Color;
use lievisual::text::{TextAlign, TextBaseline, TextStyle};

/// 文本布局包装类型
pub type TextLayout = parley::Layout<Color>;

thread_local! {
    /// 全局字体上下文 - 线程本地存储
    pub static FONT_CONTEXT: RefCell<FontContext> = RefCell::new(FontContext::default());
    /// 全局布局上下文 - 线程本地存储
    pub static LAYOUT_CONTEXT: RefCell<LayoutContext<Color>> = RefCell::new(LayoutContext::default());
}

/// 访问字体上下文的便捷函数
pub fn with_font_context<R, F: FnOnce(&mut FontContext) -> R>(f: F) -> R {
    FONT_CONTEXT.with(|cx| f(&mut cx.borrow_mut()))
}

/// 访问布局上下文的便捷函数
pub fn with_layout_context<R, F: FnOnce(&mut LayoutContext<Color>) -> R>(f: F) -> R {
    LAYOUT_CONTEXT.with(|cx| f(&mut cx.borrow_mut()))
}

/// 同时访问两个上下文的便捷函数
pub fn with_text_contexts<R, F: FnOnce(&mut FontContext, &mut LayoutContext<Color>) -> R>(
    f: F,
) -> R {
    FONT_CONTEXT.with(|font_cx| {
        LAYOUT_CONTEXT.with(|layout_cx| f(&mut font_cx.borrow_mut(), &mut layout_cx.borrow_mut()))
    })
}

/// 字体来源
#[derive(Debug, Clone)]
pub enum FontSource {
    /// 从文件路径加载
    Path(std::path::PathBuf),
    /// 从内存数据加载
    Memory(Vec<u8>),
}

/// 注册自定义字体到全局字体上下文。
///
/// 适用于不需要创建 `Chart` 实例即可加载字体的场景。
/// 加载后的字体可以通过 `font_family` 名称在图表的文本样式中使用。
///
/// # 示例
///
/// ```ignore
/// // 从内存加载（例如从 CDN 下载的字节）
/// liecharts::register_font(liecharts::FontSource::Memory(font_bytes), Some("MyFont")).unwrap();
/// ```
pub fn register_font(
    source: FontSource,
    family_name_override: Option<&str>,
) -> crate::error::DiagramResult<()> {
    use parley::fontique::Blob;

    let data = match source {
        FontSource::Path(path) => {
            let bytes = std::fs::read(&path)
                .map_err(|e| DiagramError::FontError(format!("Failed to read font file: {e}")))?;
            Blob::new(Arc::new(bytes))
        }
        FontSource::Memory(bytes) => Blob::new(Arc::new(bytes)),
    };

    let override_info = family_name_override.map(|name| parley::fontique::FontInfoOverride {
        family_name: Some(name),
        ..Default::default()
    });

    crate::text::with_font_context(|font_cx| {
        font_cx.collection.register_fonts(data, override_info);
    });

    Ok(())
}

/// 创建文本布局
///
/// 使用 parley 以 **左对齐** 排版文本，返回布局以获取自然宽度/高度。
/// 组件的对齐（居中、右对齐等）应在拿到 layout 尺寸后手动计算位置偏移。
pub fn create_text_layout(
    text: &str,
    font_config: &TextStyle,
    max_width: Option<f64>,
) -> TextLayout {
    with_text_contexts(|font_cx, layout_cx| {
        create_text_layout_with_contexts(text, font_config, max_width, font_cx, layout_cx)
    })
}

/// 使用指定的上下文创建文本布局
pub fn create_text_layout_with_contexts(
    text: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<Color>,
) -> TextLayout {
    // 创建布局构建器
    let mut builder = layout_cx.ranged_builder(font_cx, text, 1.0, true);

    // 应用样式
    let font_stack = FontFamily::named(&style.font_family);
    builder.push_default(StyleProperty::FontFamily(font_stack));
    builder.push_default(StyleProperty::FontSize(style.font_size as f32));
    builder.push_default(StyleProperty::Brush(style.color));

    // 构建布局
    let mut layout = builder.build(text);

    // 断行
    layout.break_all_lines(max_width.map(|w| w as f32));

    // 始终左对齐：parley 不做居中/右对齐，组件的对齐由 compute_text_offset 或手动计算实现
    layout.align(Alignment::Start, AlignmentOptions::default());

    layout
}

/// 将多段不同样式的文本合并在一个 TextLayout 中。
///
/// 每段文本可以有自己的 TextStyle（字体、字号、颜色）。
/// 所有文本按顺序直接拼接，通过 parley 的 RangedBuilder 为各段应用不同样式。
/// 需要换行时，请在文本段中自行包含 `\n`。
/// 最终返回单一的 TextLayout，支持断行和多行对齐。
///
/// # 参数
/// - `texts`: 文本段列表，每项为 `(文本内容, 文本样式)`。至少包含一段。
/// - `max_width`: 最大行宽，`None` 表示不断行。
/// - `align`: 多行对齐方式。`Left`、`Center` 或 `Right`。
pub fn layout_text(
    texts: &[(&str, &TextStyle)],
    max_width: Option<f64>,
    align: TextAlign,
) -> TextLayout {
    with_text_contexts(|font_cx, layout_cx| {
        layout_text_with_contexts(texts, max_width, align, font_cx, layout_cx)
    })
}

/// 使用指定的 FontContext 和 LayoutContext 创建多段样式文本布局。
///
/// 这是 `layout_text` 的低级版本，适用于需要复用上下文的场景。
/// 直接拼接所有文本段（不带额外分隔符），以第一段的样式为默认样式，
/// 其余各段通过 ranged_builder 的 `push` 方法覆盖特定范围的样式属性。
pub fn layout_text_with_contexts(
    texts: &[(&str, &TextStyle)],
    max_width: Option<f64>,
    align: TextAlign,
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<Color>,
) -> TextLayout {
    if texts.is_empty() {
        return layout_text_with_contexts(
            &[("", &TextStyle::new(Color::BLACK, 12.0, "sans-serif"))],
            max_width,
            align,
            font_cx,
            layout_cx,
        );
    }

    // 1. 直接拼接所有文本（不带额外分隔符，用户可在文本中自行添加 \n）
    let mut combined = String::new();
    let mut ranges: Vec<(usize, usize)> = Vec::with_capacity(texts.len());
    for (text, _) in texts {
        let start = combined.len();
        combined.push_str(text);
        let end = combined.len();
        ranges.push((start, end));
    }

    // 2. 创建 ranged_builder，以第一段样式作为默认
    let mut builder = layout_cx.ranged_builder(font_cx, &combined, 1.0, true);

    let first_style = &texts[0].1;
    let default_font_stack = FontFamily::named(&first_style.font_family);
    builder.push_default(StyleProperty::FontFamily(default_font_stack));
    builder.push_default(StyleProperty::FontSize(first_style.font_size as f32));
    builder.push_default(StyleProperty::Brush(first_style.color));

    // 3. 后续各段覆盖样式
    for (i, (_, style)) in texts.iter().enumerate().skip(1) {
        let (start, end) = ranges[i];
        if start >= end {
            continue;
        }
        if style.font_family != first_style.font_family {
            builder.push(
                StyleProperty::FontFamily(FontFamily::named(&style.font_family)),
                start..end,
            );
        }
        if (style.font_size - first_style.font_size).abs() > f64::EPSILON {
            builder.push(StyleProperty::FontSize(style.font_size as f32), start..end);
        }
        if style.color != first_style.color {
            builder.push(StyleProperty::Brush(style.color), start..end);
        }
    }

    // 4. 构建布局
    let mut layout = builder.build(&combined);

    // 5. 断行
    layout.break_all_lines(max_width.map(|w| w as f32));

    // 6. 对齐：映射 TextAlign → parley::Alignment
    let paragraph_align = match align {
        TextAlign::Left => Alignment::Start,
        TextAlign::Center => Alignment::Center,
        TextAlign::Right => Alignment::End,
    };
    layout.align(paragraph_align, AlignmentOptions::default());

    layout
}

/// 从锚点到文本块左上角的偏移量
///
/// 根据期望的对齐/基线方式，计算从锚点坐标到文本块左上角的偏移。
/// 组件使用范式：
///
/// ```ignore
/// let layout = create_text_layout(text, &font, max_width);
/// let (x_off, y_off) = compute_text_offset(&layout, align, baseline);
/// let position = Point::new(anchor.x + x_off, anchor.y + y_off);
///
/// // TextRun 始终用 Left/Top（position 已是左上角）
/// VisualElement::TextRun {
///     position,
///     align: TextAlign::Left,
///     baseline: TextBaseline::Top,
///     ...
/// }
/// ```
///
/// 例如：
/// - 锚点=单元格中心，align=Center → x_off = -width/2，position = 左边缘
/// - 锚点=文本中心，baseline=Middle → y_off = -height/2，position = 上边缘
pub fn compute_text_offset(
    layout: &TextLayout,
    align: TextAlign,
    baseline: TextBaseline,
) -> (f64, f64) {
    let layout_width = layout.width() as f64;
    let layout_height = layout.height() as f64;

    let x_offset = match align {
        TextAlign::Left => 0.0,
        TextAlign::Center => -layout_width / 2.0,
        TextAlign::Right => -layout_width,
    };

    let y_offset = match baseline {
        TextBaseline::Top => 0.0,
        TextBaseline::Middle => -layout_height / 2.0,
        TextBaseline::Bottom => -layout_height,
        TextBaseline::Alphabetic => {
            // 对于基线对齐，使用第一行的 ascent
            let first_line = layout.lines().next();
            if let Some(line) = first_line {
                let line_metrics = line.metrics();
                -line_metrics.ascent as f64
            } else {
                -layout_height * 0.8
            }
        }
        _ => -layout_height * 0.8,
    };

    (x_offset, y_offset)
}

/// 文本布局引擎
pub struct TextEngine;

impl TextEngine {
    pub fn new() -> Self {
        Self
    }

    /// 计算文本布局尺寸（宽度和高度）
    pub fn measure_text(
        text: &str,
        font_size: f64,
        font_family: &str,
        color: &Color,
        max_width: Option<f64>,
    ) -> (f64, f64) {
        let style = TextStyle::new(*color, font_size, font_family.to_string()).with_weight(400.0);
        let layout = create_text_layout(text, &style, max_width);
        (layout.width() as f64, layout.height() as f64)
    }
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}
