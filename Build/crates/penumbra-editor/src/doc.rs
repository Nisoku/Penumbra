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
        let blocks = penumbra_markdown::parser::parse_blocks_offsets(source)
            .unwrap_or_default()
            .into_iter()
            .map(|(md_block, source_range)| {
                let spans = extract_spans(&md_block.kind);
                Block {
                    id: md_block.id,
                    kind: md_block.kind,
                    source_range,
                    spans,
                }
            })
            .collect();
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
