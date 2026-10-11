//! Persisted quick-access folder pins on Windows.
//!
//! Config is line-based because Windows paths cannot contain LF/CR. A future
//! cross-platform version should use an escaping/serialization format instead.

use std::{
    collections::HashSet,
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};

pub fn config_path() -> io::Result<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "Pasta de configuração indisponível"))?;
    Ok(dir.join("RustFiles").join("pinned-folders.txt"))
}

/// Load saved folder paths, ignoring missing entries (removable volumes).
pub fn load(path: &Path) -> io::Result<Vec<PathBuf>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut seen = HashSet::new();
    Ok(contents
        .lines()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .filter(|path| seen.insert(path.clone()))
        .collect())
}

/// Store the complete pin set. Windows directory names cannot contain linefeeds.
pub fn save(path: &Path, pins: &[PathBuf]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(ErrorKind::InvalidInput, "Arquivo de configuração sem pasta pai")
    })?;
    fs::create_dir_all(parent)?;
    let mut contents = String::new();
    for pinned in pins {
        if pinned.as_os_str().is_empty() {
            continue;
        }
        let path = pinned.to_str().ok_or_else(|| {
            io::Error::new(ErrorKind::InvalidData, "Caminho não é texto Unicode")
        })?;
        if path.contains(['\n', '\r']) {
            return Err(io::Error::new(ErrorKind::InvalidData, "Caminho contém quebra de linha"));
        }
        contents.push_str(path);
        contents.push('\n');
    }
    fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn stores_deduplicated_existing_paths() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("primeira");
        fs::create_dir(&first).unwrap();
        let config = dir.path().join("settings/pins.txt");
        save(&config, &[first.clone(), first.clone()]).unwrap();
        assert_eq!(load(&config).unwrap(), vec![first]);
    }

    #[test]
    fn missing_configuration_returns_empty() {
        let dir = tempdir().unwrap();
        assert!(load(&dir.path().join("missing.txt")).unwrap().is_empty());
    }
}
