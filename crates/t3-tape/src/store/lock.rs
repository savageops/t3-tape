use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use fs2::FileExt;

use crate::exit::RedtapeError;

#[derive(Debug)]
pub struct StateLock {
    _path: PathBuf,
    file: std::fs::File,
}

impl StateLock {
    pub fn acquire(path: &Path) -> Result<Self, RedtapeError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        match file.try_lock_exclusive() {
            Ok(()) => Ok(Self {
                _path: path.to_path_buf(),
                file,
            }),
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.raw_os_error() == Some(33) =>
            {
                Err(RedtapeError::Blocked(
                    "state lock held: another t3-tape process is running".to_string(),
                ))
            }
            Err(err) => Err(err.into()),
        }
    }
}

impl Drop for StateLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}
