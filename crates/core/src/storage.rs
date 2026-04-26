use std::path::PathBuf;

pub struct Storage {
    base_path: PathBuf,
}

impl Storage {
    pub fn new(base: PathBuf) -> Self {
        Self { base_path: base }
    }

    pub fn notes_dir(&self) -> PathBuf {
        self.base_path.join("notes")
    }

    pub fn index_dir(&self) -> PathBuf {
        self.base_path.join("index")
    }
}