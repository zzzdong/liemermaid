/// 字体粗细命名值
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontWeightNamed {
    Normal,
    Bold,
    Bolder,
    Lighter,
}

/// 字体粗细
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontWeight {
    Named(FontWeightNamed),
    Numeric(u16),
}

impl Default for FontWeight {
    fn default() -> Self {
        FontWeight::Named(FontWeightNamed::Normal)
    }
}
