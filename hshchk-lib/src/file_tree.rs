use std::io::Result;
use std::{fs};
use std::path::{Path, PathBuf};

pub trait FileTreeProcessor {
    fn process_file(&mut self, file_path: &PathBuf);
}

pub struct FileTree<'a, T: FileTreeProcessor> {
    processor: &'a mut T
}

impl<'a, T: FileTreeProcessor> FileTree<'a, T> {
    pub fn new(processor: &'a mut T) -> Self {
        FileTree {
            processor
        }
    }
    pub fn traverse(&mut self, path: &Path) -> Result<()> {
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    self.traverse(&path)?;
                } else {
                    self.processor.process_file(&entry.path());
                }
            }
        }
        Ok(())        
    }
}