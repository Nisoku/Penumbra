mod builder;
mod scanner;
mod tags;

use crate::ast::{Block, Document};
use builder::Ctx;
use penumbra_core::error::Result;

pub fn parse_document(text: &str) -> Result<Document> {
    let (blocks, _) = run_offsets(text)?;
    Ok(Document { blocks })
}

pub fn parse_block(text: &str) -> Result<Vec<Block>> {
    Ok(run_offsets(text)?.0)
}

/// A half-open byte range of a block in the parsed source text.
pub type SourceRange = (usize, usize);

/// Parse into top-level blocks paired with their byte range in the source.
pub fn parse_blocks_offsets(text: &str) -> Result<Vec<(Block, SourceRange)>> {
    let (blocks, ranges) = run_offsets(text)?;
    debug_assert_eq!(blocks.len(), ranges.len());
    Ok(blocks.into_iter().zip(ranges).collect())
}

fn run_offsets(text: &str) -> Result<(Vec<Block>, Vec<SourceRange>)> {
    let options = pulldown_cmark::Options::ENABLE_TABLES
        | pulldown_cmark::Options::ENABLE_FOOTNOTES
        | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
        | pulldown_cmark::Options::ENABLE_TASKLISTS
        | pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES;

    let parser = pulldown_cmark::Parser::new_ext(text, options);
    let mut ctx = Ctx::new();
    for (event, range) in parser.into_offset_iter() {
        ctx.handle(event, &range)?;
        ctx.last_end = ctx.last_end.max(range.end);
    }
    ctx.finish()
}

pub fn markdown_to_html(text: &str) -> Result<String> {
    let doc = parse_document(text)?;
    Ok(crate::render::html::render_html(&doc))
}

pub fn markdown_to_plain(text: &str) -> Result<String> {
    let doc = parse_document(text)?;
    Ok(doc.plain_text())
}
