use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cursor {
    offset: usize,
}

impl Cursor {
    pub fn new(offset: usize) -> Self {
        Self { offset }
    }

    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn set_offset(&mut self, n: usize) {
        self.offset = n;
    }

    pub fn move_left(&mut self) {
        self.offset = self.offset.saturating_sub(1);
    }

    pub fn move_right(&mut self, source_len: usize) {
        if self.offset < source_len {
            self.offset += 1;
        }
    }

    #[must_use]
    pub fn at_start(&self) -> bool {
        self.offset == 0
    }

    #[must_use]
    pub fn at_end(&self, source_len: usize) -> bool {
        self.offset >= source_len
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    #[must_use]
    pub fn collapsed(&self) -> bool {
        self.anchor == self.head
    }

    #[must_use]
    pub fn range(&self) -> (usize, usize) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    #[must_use]
    pub fn invert(&self) -> Selection {
        Self {
            anchor: self.head,
            head: self.anchor,
        }
    }
}
