use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::doc::BlockId;

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("offset {0} out of bounds (len {1})")]
    OffsetOutOfBounds(usize, usize),

    #[error("block {0} not found")]
    BlockNotFound(String),

    #[error("invalid range {0}..{1}")]
    InvalidRange(usize, usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockMark {
    Bold,
    Italic,
    Strikethrough,
    Code,
}

impl BlockMark {
    #[must_use]
    pub fn markers(&self) -> &'static str {
        match self {
            BlockMark::Bold => "**",
            BlockMark::Italic => "*",
            BlockMark::Strikethrough => "~~",
            BlockMark::Code => "`",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    InsertText { at: usize, text: String },
    DeleteRange { from: usize, to: usize },
    SplitBlock { at: usize },
    MergeBlocks { first: BlockId },
    SetMark { block: BlockId, mark: BlockMark },
    MoveCursor { to: usize },
}

#[derive(Debug, Clone)]
pub struct Journal {
    commands: Vec<Command>,
    pointer: usize,
}

impl Journal {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            pointer: 0,
        }
    }

    pub fn push(&mut self, cmd: Command) {
        self.commands.truncate(self.pointer);
        self.commands.push(cmd);
        self.pointer = self.commands.len();
    }

    #[must_use]
    pub fn undo(&mut self) -> Option<Command> {
        if self.pointer == 0 {
            return None;
        }
        self.pointer -= 1;
        self.commands.get(self.pointer).cloned()
    }

    #[must_use]
    pub fn redo(&mut self) -> Option<Command> {
        if self.pointer >= self.commands.len() {
            return None;
        }
        let cmd = self.commands.get(self.pointer).cloned();
        self.pointer += 1;
        cmd
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.pointer > 0
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.pointer < self.commands.len()
    }
}

impl Default for Journal {
    fn default() -> Self {
        Self::new()
    }
}

pub fn apply_command(source: &str, cmd: &Command) -> Result<String, CommandError> {
    let len = source.len();
    match cmd {
        Command::InsertText { at, text } => {
            if *at > len {
                return Err(CommandError::OffsetOutOfBounds(*at, len));
            }
            let mut result = String::with_capacity(len + text.len());
            result.push_str(&source[..*at]);
            result.push_str(text);
            result.push_str(&source[*at..]);
            Ok(result)
        }
        Command::DeleteRange { from, to } => {
            if *from > len || *to > len {
                return Err(CommandError::InvalidRange(*from, *to));
            }
            if *from >= *to {
                return Err(CommandError::InvalidRange(*from, *to));
            }
            let mut result = String::with_capacity(len - (*to - *from));
            result.push_str(&source[..*from]);
            result.push_str(&source[*to..]);
            Ok(result)
        }
        Command::SplitBlock { at } => {
            if *at > len {
                return Err(CommandError::OffsetOutOfBounds(*at, len));
            }
            let mut result = String::with_capacity(len + 2);
            result.push_str(&source[..*at]);
            result.push_str("\n\n");
            result.push_str(&source[*at..]);
            Ok(result)
        }
        Command::MergeBlocks { .. } => {
            // find the first double newline and remove it
            if let Some(pos) = source.find("\n\n") {
                let mut result = String::with_capacity(len - 2);
                result.push_str(&source[..pos]);
                result.push_str(&source[pos + 2..]);
                Ok(result)
            } else {
                Ok(source.to_owned())
            }
        }
        Command::SetMark { block, mark } => {
            // we need the block's source range, but we only have the source string
            // so we operate on the whole source and use a placeholder approach:
            // for now, we return the source unchanged if we can't find the block range.
            // TODO: take a Document reference.
            let _ = (block, mark);
            Ok(source.to_owned())
        }
        Command::MoveCursor { .. } => Ok(source.to_owned()),
    }
}
