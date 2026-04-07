use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiagnosticError {
    #[error("subprocess failed: {0}")]
    Subprocess(String),

    #[error("D-Bus error: {0}")]
    DBus(#[from] zbus::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("check skipped: {0}")]
    Skipped(String),
}

pub type DiagnosticResult<T> = Result<T, DiagnosticError>;
