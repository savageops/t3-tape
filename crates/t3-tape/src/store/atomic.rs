use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

use crate::exit::RedtapeError;

pub fn write_new_file_atomic(path: &Path, contents: &[u8]) -> Result<(), RedtapeError> {
    if path.exists() {
        return Err(RedtapeError::Usage(format!(
            "refusing to overwrite existing path: {}",
            path.display()
        )));
    }

    let parent = path.parent().ok_or_else(|| {
        RedtapeError::Usage(format!("path has no parent directory: {}", path.display()))
    })?;

    fs::create_dir_all(parent)?;

    let temp_path = temporary_path_for(path);
    write_temp_file(&temp_path, contents)?;
    move_new_file(&temp_path, path)?;
    Ok(())
}

pub fn write_file_atomic(path: &Path, contents: &[u8]) -> Result<(), RedtapeError> {
    let parent = path.parent().ok_or_else(|| {
        RedtapeError::Usage(format!("path has no parent directory: {}", path.display()))
    })?;

    fs::create_dir_all(parent)?;

    let temp_path = temporary_path_for(path);
    write_temp_file(&temp_path, contents)?;

    let result = if path.exists() {
        replace_existing_file(&temp_path, path)
    } else {
        move_new_file(&temp_path, path)
    };

    result.map_err(|err| {
        let _ = fs::remove_file(&temp_path);
        err.into()
    })
}

pub fn append_lines(path: &Path, lines: &[String]) -> Result<(), RedtapeError> {
    let parent = path.parent().ok_or_else(|| {
        RedtapeError::Usage(format!("path has no parent directory: {}", path.display()))
    })?;
    fs::create_dir_all(parent)?;

    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(path)?;

    let len = file.metadata()?.len();
    if len > 0 {
        file.seek(SeekFrom::End(-1))?;
        let mut last = [0u8; 1];
        file.read_exact(&mut last)?;
        if last[0] != b'\n' {
            file.write_all(b"\n")?;
        }
    }

    file.write_all(lines.join("\n").as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn write_temp_file(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;

    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    Ok(())
}

fn move_new_file(temp_path: &Path, target_path: &Path) -> std::io::Result<()> {
    if let Err(err) = fs::rename(temp_path, target_path) {
        let _ = fs::remove_file(temp_path);
        return Err(err);
    }

    Ok(())
}

#[cfg(windows)]
fn replace_existing_file(temp_path: &Path, target_path: &Path) -> std::io::Result<()> {
    let replacement = encode_wide(temp_path);
    let replaced = encode_wide(target_path);

    let status = unsafe {
        ReplaceFileW(
            replaced.as_ptr(),
            replacement.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };

    if status == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn encode_wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(not(windows))]
fn replace_existing_file(temp_path: &Path, target_path: &Path) -> std::io::Result<()> {
    fs::rename(temp_path, target_path)
}

fn temporary_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("t3-tape"));
    let mut temp_name = OsString::from(".");
    temp_name.push(file_name);
    temp_name.push(format!(".tmp-{}-{}", process::id(), unique_suffix()));
    path.with_file_name(temp_name)
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
