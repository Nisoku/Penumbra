use thiserror::Error;

#[derive(Error, Debug)]
pub enum PenumbraError {
    #[error("note not found: {0}")]
    NoteNotFound(String),

    #[error("graph error: {0}")]
    Graph(String),

    #[error("layout error: {0}")]
    Layout(String),

    #[error("embedding error: {0}")]
    Embedding(String),

    #[error("search error: {0}")]
    Search(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("sync error: {0}")]
    Sync(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, PenumbraError>;
