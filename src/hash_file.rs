use crate::{replaceable_separator, HashFileFormat};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Component, Path, MAIN_SEPARATOR_STR};
use tokio_util::sync::CancellationToken;

const MAX_PATH_SIZE: usize = 4_095;
const MAX_HASH_SIZE: usize = 1024;

#[derive(Clone)]
pub struct HashFileEntry {
    pub file_path: String,
    pub size: Option<u64>,
    pub digest: String,
}

pub struct HashFile {
    files: HashMap<String, HashFileEntry>,
}

impl HashFile {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Parse separately so a malformed manifest cannot be partly loaded.
    pub fn load(&mut self, file_path: &Path) -> io::Result<()> {
        let reader = BufReader::new(File::open(file_path)?);
        let mut files = HashMap::new();
        let mut format = None;
        for (index, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let entry_format = *format.get_or_insert_with(|| infer_format(&line));
            let entry = parse_entry(&line, entry_format).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{}:{}: {}", file_path.display(), index + 1, error),
                )
            })?;
            if files.insert(entry.file_path.clone(), entry).is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{}:{}: duplicate file entry",
                        file_path.display(),
                        index + 1
                    ),
                ));
            }
        }
        self.files = files;
        Ok(())
    }

    pub fn detect_format(path: &Path) -> io::Result<Option<HashFileFormat>> {
        for line in BufReader::new(File::open(path)?).lines() {
            let line = line?;
            if !line.trim().is_empty() {
                return Ok(Some(infer_format(&line)));
            }
        }
        Ok(None)
    }

    /// Build the replacement beside the destination; never truncate the original.
    pub fn save(
        &self,
        file_path: &Path,
        format: HashFileFormat,
        token: &CancellationToken,
        overwrite: bool,
    ) -> io::Result<()> {
        let parent = file_path.parent().unwrap_or_else(|| Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        {
            let mut writer = BufWriter::new(temp.as_file_mut());
            let mut entries: Vec<_> = self.files.values().collect();
            entries.sort_by(|a, b| a.file_path.cmp(&b.file_path));
            for entry in entries {
                if token.is_cancelled() {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "save canceled"));
                }
                match format {
                    HashFileFormat::HashCheck => writeln!(
                        writer,
                        "{}|{}|{}",
                        entry.file_path,
                        entry
                            .size
                            .ok_or_else(|| invalid("entry has no recorded size"))?,
                        entry.digest
                    )?,
                    HashFileFormat::HashSum => {
                        writeln!(writer, "{} *{}", entry.digest, entry.file_path)?
                    }
                }
            }
            writer.flush()?;
        }
        if overwrite {
            match std::fs::metadata(file_path) {
                Ok(metadata) => temp.as_file().set_permissions(metadata.permissions())?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        temp.as_file().sync_all()?;
        if token.is_cancelled() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "save canceled"));
        }
        if overwrite {
            temp.persist(file_path).map_err(|error| error.error)?;
        } else {
            temp.persist_noclobber(file_path)
                .map_err(|error| error.error)?;
        }
        Ok(())
    }

    pub fn add_entry(&mut self, entry: HashFileEntry) {
        self.files.insert(entry.file_path.clone(), entry);
    }
    pub fn remove_entry(&mut self, path: &str) {
        self.files.remove(path);
    }
    pub fn get_entry(&self, path: &str) -> Option<&HashFileEntry> {
        self.files.get(path)
    }
    pub fn get_file_paths(&self) -> Vec<String> {
        self.files.keys().cloned().collect()
    }
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

fn infer_format(line: &str) -> HashFileFormat {
    if line.split_once(' ').is_some_and(|(digest, rest)| {
        matches!(digest.len(), 32 | 40 | 64 | 128)
            && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            && (rest.starts_with('*') || rest.starts_with(' '))
    }) {
        HashFileFormat::HashSum
    } else if line.contains('|') {
        HashFileFormat::HashCheck
    } else {
        HashFileFormat::HashSum
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn parse_entry(line: &str, format: HashFileFormat) -> io::Result<HashFileEntry> {
    let (path, size, digest) = match format {
        HashFileFormat::HashCheck => {
            let parts: Vec<_> = line.split('|').collect();
            if parts.len() != 3 {
                return Err(invalid("expected path|size|digest"));
            }
            let size = parts[1]
                .parse::<u64>()
                .map_err(|_| invalid("invalid file size"))?;
            (parts[0], Some(size), parts[2])
        }
        HashFileFormat::HashSum => {
            let (digest, rest) = line
                .split_once(' ')
                .ok_or_else(|| invalid("expected digest and file path"))?;
            let path = rest
                .strip_prefix('*')
                .or_else(|| rest.strip_prefix(' '))
                .ok_or_else(|| invalid("expected binary or text checksum marker"))?;
            (path, None, digest)
        }
    };
    let path = path.replace(replaceable_separator(), MAIN_SEPARATOR_STR);
    if path.is_empty() || path.len() > MAX_PATH_SIZE {
        return Err(invalid("invalid file path length"));
    }
    let mut normalized = std::path::PathBuf::new();
    for component in Path::new(&path).components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            _ => return Err(invalid("file path must stay within the target directory")),
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(invalid("empty file path"));
    }
    if digest.is_empty() || digest.len() > MAX_HASH_SIZE {
        return Err(invalid("invalid digest length"));
    }
    Ok(HashFileEntry {
        file_path: normalized.to_string_lossy().into_owned(),
        size,
        digest: digest.to_lowercase(),
    })
}
#[cfg(test)]
#[path = "tests/hash_file.rs"]
mod tests;
