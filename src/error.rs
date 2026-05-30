use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("pest parse error: {0}")]
    Pest(#[from] Box<pest::error::Error<super::parser::Rule>>),

    #[error("no diagram found in input")]
    NoDiagram,

    #[error("unsupported diagram type")]
    UnsupportedDiagram,

    #[error("invalid syntax at line {line}, column {col}: {message}")]
    InvalidSyntax { line: usize, col: usize, message: String },
}

#[derive(Error, Debug)]
pub enum DiagramError {
    #[error("unsupported diagram type: {0}")]
    UnsupportedType(String),

    #[error("layout error: {0}")]
    LayoutError(String),

    #[error("text layout error: {0}")]
    TextLayoutError(String),

    #[error("font error: {0}")]
    FontError(String),

    #[error("render error: {0}")]
    RenderError(String),
}

pub type ParseResult<T> = std::result::Result<T, ParseError>;
pub type DiagramResult<T> = std::result::Result<T, DiagramError>;