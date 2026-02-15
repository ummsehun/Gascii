use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
pub fn list_files(dir: &str, extension: &str) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory: {}", dir))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file() && path.extension().map_or(false, |ext| ext == extension)
        })
        .collect();

    // Sort alphabetically (works for padded numbers like frame_00001.bin)
    files.sort();

    if files.is_empty() {
        anyhow::bail!("No files with extension '{}' found in '{}'", extension, dir);
    }

    Ok(files)
}

pub fn read_file(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).with_context(|| format!("Failed to read file: {:?}", path))
}
