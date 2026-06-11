pub mod embed;
pub mod error;
pub mod link;
pub mod note;
pub mod position;
pub mod tag;

pub use embed::EmbeddingProvider;
pub use error::{PenumbraError, Result};
pub use link::{Link, LinkKind};
pub use note::{Note, NoteId, NoteMeta};
pub use position::{Bounds, Position};
pub use tag::Tag;
