use cancellation::CancellationToken;
use std::fs;
use std::io::Result;
use std::path::Path;
use std::sync::Arc;

pub trait FileTreeProcessor {
    fn process_file(&mut self, file_path: &Path);
}

pub struct FileTree<'a, T: FileTreeProcessor> {
    processor: &'a mut T,
}

impl<'a, T: FileTreeProcessor> FileTree<'a, T> {
    pub fn new(processor: &'a mut T) -> Self {
        FileTree { processor }
    }
    pub fn traverse(
        &mut self,
        path: &Path,
        cancellation_token: &Arc<CancellationToken>,
    ) -> Result<()> {
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                if cancellation_token.is_canceled() {
                    break;
                }

                let path = entry?.path();
                if path.is_dir() {
                    self.traverse(&path, cancellation_token)?;
                } else {
                    self.processor.process_file(&path);
                }
            }
        }

        Ok(())
    }
}
