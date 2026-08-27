use serde::{Deserialize, Serialize};

use crate::cursor::Cursor;
use crate::doc::{BlockId, BlockKind, Document, StyledSpan};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockSnapshot {
    pub id: BlockId,
    pub kind: BlockKind,
    pub spans: Vec<StyledSpan>,
    pub source_range: (usize, usize),
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct ViewModel {
    blocks: Vec<BlockSnapshot>,
    cursor: Cursor,
}

impl ViewModel {
    pub fn from_doc(doc: &Document, active_block: Option<BlockId>, cursor: Cursor) -> Self {
        let blocks = doc
            .blocks
            .iter()
            .map(|b| BlockSnapshot {
                id: b.id,
                kind: b.kind.clone(),
                spans: b.spans.clone(),
                source_range: b.source_range,
                is_active: active_block == Some(b.id),
            })
            .collect();

        Self { blocks, cursor }
    }

    #[must_use]
    pub fn blocks(&self) -> &[BlockSnapshot] {
        &self.blocks
    }

    #[must_use]
    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    #[must_use]
    pub fn active_block(&self) -> Option<BlockId> {
        self.blocks.iter().find(|b| b.is_active).map(|b| b.id)
    }
}
