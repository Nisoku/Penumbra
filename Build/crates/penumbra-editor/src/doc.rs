use serde::{Deserialize, Serialize};

pub use penumbra_markdown::ast::{BlockId, BlockKind};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyledSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub code: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: BlockId,
    pub kind: BlockKind,
    pub source_range: (usize, usize),
    pub spans: Vec<StyledSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub source: String,
    pub blocks: Vec<Block>,
}

impl Document {
    pub fn new(source: &str) -> Self {
        let md_doc = penumbra_markdown::parser::parse_document(source)
            .unwrap_or_else(|_| penumbra_markdown::ast::Document::empty());
        let blocks = convert_blocks(&md_doc.blocks, source);
        Self {
            source: source.to_owned(),
            blocks,
        }
    }

    pub fn block(&self, id: BlockId) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }

    #[must_use]
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn source_range(&self, id: BlockId) -> Option<(usize, usize)> {
        self.block(id).map(|b| b.source_range)
    }
}

fn convert_blocks(md_blocks: &[penumbra_markdown::ast::Block], source: &str) -> Vec<Block> {
    let mut offset = 0;
    let mut result = Vec::new();

    for md_block in md_blocks {
        let (start, end) = find_block_range(source, offset, md_block);
        let spans = extract_spans(&md_block.kind);

        result.push(Block {
            id: md_block.id,
            kind: md_block.kind.clone(),
            source_range: (start, end),
            spans,
        });

        offset = end;
    }

    result
}

fn find_block_range(
    source: &str,
    hint: usize,
    _md_block: &penumbra_markdown::ast::Block,
) -> (usize, usize) {
    let bytes = source.as_bytes();
    let len = bytes.len();
    if hint >= len {
        return (len, len);
    }

    let start = hint;
    let mut end = start;

    while end < len {
        if bytes[end] == b'\n' {
            if end + 1 < len && bytes[end + 1] == b'\n' {
                end += 2;
                break;
            }
            end += 1;
        } else {
            end += 1;
        }
    }

    (start, end)
}

fn extract_spans(md_block: &penumbra_markdown::ast::BlockKind) -> Vec<StyledSpan> {
    let mut spans = Vec::new();
    match md_block {
        penumbra_markdown::ast::BlockKind::Paragraph(inlines)
        | penumbra_markdown::ast::BlockKind::Heading {
            children: inlines, ..
        } => {
            extract_inline_spans(inlines, &mut spans, false, false, false);
        }
        penumbra_markdown::ast::BlockKind::Quote(children) => {
            for child in children {
                spans.extend(extract_spans(&child.kind));
            }
        }
        penumbra_markdown::ast::BlockKind::CodeBlock { text, .. } => {
            spans.push(StyledSpan {
                text: text.clone(),
                bold: false,
                italic: false,
                strikethrough: false,
                code: true,
            });
        }
        _ => {}
    }
    spans
}

fn extract_inline_spans(
    inlines: &[penumbra_markdown::ast::Inline],
    out: &mut Vec<StyledSpan>,
    bold: bool,
    italic: bool,
    strikethrough: bool,
) {
    for inline in inlines {
        match inline {
            penumbra_markdown::ast::Inline::Text(t) => {
                out.push(StyledSpan {
                    text: t.clone(),
                    bold,
                    italic,
                    strikethrough,
                    code: false,
                });
            }
            penumbra_markdown::ast::Inline::Code(t) => {
                out.push(StyledSpan {
                    text: t.clone(),
                    bold,
                    italic,
                    strikethrough,
                    code: true,
                });
            }
            penumbra_markdown::ast::Inline::Strong(children) => {
                extract_inline_spans(children, out, true, italic, strikethrough);
            }
            penumbra_markdown::ast::Inline::Emphasis(children) => {
                extract_inline_spans(children, out, bold, true, strikethrough);
            }
            penumbra_markdown::ast::Inline::Strikethrough(children) => {
                extract_inline_spans(children, out, bold, italic, true);
            }
            penumbra_markdown::ast::Inline::Link { children, .. } => {
                extract_inline_spans(children, out, bold, italic, strikethrough);
            }
            _ => {}
        }
    }
}
