use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum AppError {
    #[error("Invalid YouTube URL or video ID")]
    InvalidInput,

    #[error("API error: {0}")]
    Api(String),

    #[error("I/O error: {0}")]
    Io(String),
}
