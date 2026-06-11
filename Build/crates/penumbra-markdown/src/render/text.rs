use crate::ast::Document;

/// Extract plain text from a Document AST.
///
/// Strips all formatting and custom elements, returning only
/// the raw text content. Useful for embedding generation and
/// full-text search indexing.
pub fn render_plain(doc: &Document) -> String {
    doc.plain_text()
}
