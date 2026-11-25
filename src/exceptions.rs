use thiserror::Error;

/// Error type returned across `myuri-rs`.
#[derive(Debug, Error)]
pub enum MyuriError {
    /// A click could not reach its element; contains the miss reason.
    #[error("Missed: {0}")]
    Missed(String),

    #[error("myuri: unknown os {0:?}; use 'macos', 'windows' or 'linux'")]
    UnknownOs(String),

    #[error("myuri: unknown setting '{0}'")]
    UnknownSetting(String),

    #[error("myuri: {0}")]
    InvalidArgument(String),

    #[error("myuri: {0}")]
    DriverError(String),
}

pub type Result<T> = std::result::Result<T, MyuriError>;
