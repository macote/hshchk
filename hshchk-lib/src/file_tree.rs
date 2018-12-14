use std::io::Result;
use std::{fs};
use std::path::{Path, PathBuf};

pub trait FileTreeProcessor {
    fn process_file(&self, file_path: &PathBuf);
}

pub struct FileTree<'a, T: FileTreeProcessor> {
    processor: &'a T
}

impl<'a, T: FileTreeProcessor> FileTree<'a, T> {
    pub fn new(processor: &'a T) -> Self {
        FileTree {
            processor
        }
    }
    pub fn traverse(self, base_path: &str) -> Result<()> {
        let path = Path::new(base_path);
        self.visit_dir_entry(path)
    }
    fn visit_dir_entry(&self, dir: &Path) -> Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    self.visit_dir_entry(&path)?;
                } else {
                    self.processor.process_file(&entry.path());
                }
            }
        }
        Ok(())        
    }
}