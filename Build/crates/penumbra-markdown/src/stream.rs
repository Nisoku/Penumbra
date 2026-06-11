use mdstream::{DocumentState, MdStream, Options as MdStreamOptions, Update};
use penumbra_core::error::Result;

use crate::ast::{Block, Document};
use crate::parser::parse_block;

pub struct MarkdownStream {
    stream: MdStream,
    state: DocumentState,
    /// mdstream block id → vector of AST blocks
    committed: Vec<(mdstream::BlockId, Vec<Block>)>,
}

impl MarkdownStream {
    pub fn new() -> Self {
        Self::with_options(MdStreamOptions::default())
    }

    pub fn with_options(opts: MdStreamOptions) -> Self {
        Self {
            stream: MdStream::new(opts),
            state: DocumentState::new(),
            committed: Vec::new(),
        }
    }

    pub fn append(&mut self, chunk: &str) -> Result<StreamUpdate> {
        let update = self.stream.append(chunk);
        self.apply_update(update)
    }

    pub fn finalize(&mut self) -> Result<StreamUpdate> {
        let update = self.stream.finalize();
        self.apply_update(update)
    }

    pub fn snapshot(&self) -> Document {
        let mut blocks = Vec::new();
        for (_, b) in &self.committed {
            blocks.extend_from_slice(b);
        }
        Document { blocks }
    }

    pub fn pending_raw(&self) -> Option<&str> {
        self.state.pending().map(|b| b.raw.as_str())
    }

    pub fn reset(&mut self) {
        self.stream.reset();
        self.state = DocumentState::new();
        self.committed.clear();
    }

    fn apply_update(&mut self, update: Update) -> Result<StreamUpdate> {
        let applied = self.state.apply(update);
        let committed_before = self.committed.len();
        let current = self.state.committed();

        let mut new: Vec<Block> = Vec::new();

        for block in current.iter().skip(committed_before) {
            let ast = parse_block(&block.raw)?;
            new.extend_from_slice(&ast);
            self.committed.push((block.id, ast));
        }

        if !applied.invalidated.is_empty() {
            let invalid: Vec<_> = applied.invalidated.to_vec();
            self.committed.retain(|(id, _)| !invalid.contains(id));
            for block in current {
                if !self.committed.iter().any(|(id, _)| *id == block.id) {
                    let ast = parse_block(&block.raw)?;
                    new.extend_from_slice(&ast);
                    self.committed.push((block.id, ast));
                }
            }
        }

        if applied.reset {
            self.committed.clear();
            new.clear();
            for block in current {
                let ast = parse_block(&block.raw)?;
                new.extend_from_slice(&ast);
                self.committed.push((block.id, ast));
            }
        }

        let pending = self.state.pending().map(|p| PendingInfo {
            id: p.id,
            raw: p.raw.clone(),
            display: p.display.clone(),
        });

        Ok(StreamUpdate {
            committed: new,
            pending,
            reset: applied.reset,
        })
    }
}

impl Default for MarkdownStream {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct StreamUpdate {
    pub committed: Vec<Block>,
    pub pending: Option<PendingInfo>,
    pub reset: bool,
}

#[derive(Debug, Clone)]
pub struct PendingInfo {
    pub id: mdstream::BlockId,
    pub raw: String,
    pub display: Option<String>,
}

impl PendingInfo {
    pub fn display_or_raw(&self) -> &str {
        self.display.as_deref().unwrap_or(&self.raw)
    }
}
