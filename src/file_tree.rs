use std::fs;
use std::io::Result;
use std::path::Path;
use tokio_util::sync::CancellationToken;

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
    pub fn traverse(&mut self, path: &Path, cancellation_token: &CancellationToken) -> Result<()> {
        for entry in fs::read_dir(path)? {
            if cancellation_token.is_cancelled() {
                break;
            }

            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_symlink() && path.is_dir() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    format!("directory symlinks are not followed: {}", path.display()),
                ));
            }
            if file_type.is_dir() {
                self.traverse(&path, cancellation_token)?;
            } else {
                self.processor.process_file(&path);
            }
        }

        Ok(())
    }
}
