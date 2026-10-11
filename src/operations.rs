//! Windows-first, explicit filesystem mutations.
//!
//! No implicit overwrite and no fallback to copy+delete for cross-volume moves.
//! These operations are not transactional: callers must surface failures to users.

use std::{
    ffi::OsStr,
    fs::{self, OpenOptions},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use crate::filesystem::{self, EntryKind, FileEntry};

/// Validate a single Windows-compatible file name rather than a path.
pub fn validate_name(name: &str) -> io::Result<()> {
    if name.is_empty() || name == "." || name == ".." || name.trim() != name {
        return Err(io::Error::new(ErrorKind::InvalidInput, "Nome de arquivo inválido"));
    }
    if name.ends_with('.') || name.chars().any(|c| {
        c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
    }) {
        return Err(io::Error::new(ErrorKind::InvalidInput, "O nome contém caracteres proibidos"));
    }
    let base = name.split('.').next().unwrap_or("").trim_end_matches(' ').to_ascii_uppercase();
    if matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4 && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(io::Error::new(ErrorKind::InvalidInput, "Nome reservado pelo Windows"));
    }
    Ok(())
}

/// Rename a file/folder in place. Windows' rename fails if the destination exists.
/// There is no implicit overwrite; on other platforms the preflight alone is not
/// sufficient protection against a concurrent creator.
pub fn rename_item(source: &Path, new_name: &str) -> io::Result<PathBuf> {
    validate_name(new_name)?;
    fs::symlink_metadata(source)?;
    let parent = source.parent().ok_or_else(|| {
        io::Error::new(ErrorKind::InvalidInput, "Origem sem diretório pai")
    })?;
    let destination = parent.join(new_name);
    if destination == source {
        return Ok(source.to_path_buf());
    }
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(io::Error::new(ErrorKind::AlreadyExists, "Já existe um item com esse nome"));
    }
    fs::rename(source, &destination)?;
    Ok(destination)
}

/// Moves on the same volume without an implicit recursive copy/delete.
/// Cross-volume moves are reported instead of deleting the source after a partial copy.
pub fn move_into(source: &Path, destination_directory: &Path) -> io::Result<PathBuf> {
    if !destination_directory.is_dir() {
        return Err(io::Error::new(ErrorKind::InvalidInput, "O destino não é uma pasta"));
    }
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(ErrorKind::InvalidInput, "Mover links simbólicos não é suportado"));
    }
    let source_parent = source.parent().ok_or_else(|| {
        io::Error::new(ErrorKind::InvalidInput, "Caminho de origem inválido")
    })?;
    if fs::canonicalize(source_parent)? == fs::canonicalize(destination_directory)? {
        return Ok(source.to_path_buf());
    }
    if metadata.is_dir() {
        let canonical_source = fs::canonicalize(source)?;
        let canonical_destination = fs::canonicalize(destination_directory)?;
        if canonical_destination.starts_with(&canonical_source) {
            return Err(io::Error::new(ErrorKind::InvalidInput, "Uma pasta não pode ser movida para dentro dela mesma"));
        }
    }
    let name = source.file_name().ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "Nome inválido"))?;
    let destination = filesystem::unique_copy_path(destination_directory, name, metadata.is_dir())?;
    fs::rename(source, &destination)?;
    Ok(destination)
}

fn spreadsheet_safe(value: &str) -> String {
    if matches!(value.chars().next(), Some('=' | '+' | '-' | '@' | '\t' | '\r')) {
        format!("'{value}")
    } else {
        value.to_owned()
    }
}

fn csv_field(value: &str) -> String {
    let sanitized = spreadsheet_safe(value);
    format!("\"{}\"", sanitized.replace('"', "\"\""))
}

/// Write a UTF-8 BOM CSV with Windows line endings; never overwrite an existing export.
/// The 'Modified Unix' column contains Unix seconds (blank if unavailable).
pub fn export_csv(directory: &Path, entries: &[FileEntry]) -> io::Result<PathBuf> {
    if !directory.is_dir() {
        return Err(io::Error::new(ErrorKind::InvalidInput, "O destino não é uma pasta"));
    }
    let path = filesystem::unique_copy_path(directory, OsStr::new("files-export.csv"), false)?;
    let mut output = OpenOptions::new().write(true).create_new(true).open(&path)?;
    output.write_all(b"\xEF\xBB\xBFName,Type,Size (bytes),Modified (Unix seconds)\r\n")?;
    for entry in entries {
        let kind = match entry.kind {
            EntryKind::Folder => "Folder",
            EntryKind::File => "File",
            EntryKind::Symlink => "Symlink",
        };
        let modified = entry.modified.and_then(|value| {
            value.duration_since(UNIX_EPOCH).ok().map(|duration| duration.as_secs().to_string())
        }).unwrap_or_default();
        writeln!(output, "{},{},{},{}\r", csv_field(&entry.name), csv_field(kind), entry.size, modified)?;
    }
    output.flush()?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn refuses_reserved_and_traversal_names() {
        for name in ["..", "CON.txt", "LPT1", "bad/name", "a\\b", "a:", "file.", "=bad?"] {
            assert!(validate_name(name).is_err(), "{name}");
        }
        assert!(validate_name("relatório 2026.txt").is_ok());
    }

    #[test]
    fn rename_rejects_existing_target() {
        let tmp = tempdir().unwrap();
        let a = tmp.path().join("a.txt");
        let b = tmp.path().join("b.txt");
        fs::write(&a, "A").unwrap();
        fs::write(&b, "B").unwrap();
        assert_eq!(rename_item(&a, "b.txt").unwrap_err().kind(), ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&a).unwrap(), "A");
        assert_eq!(fs::read_to_string(&b).unwrap(), "B");
    }

    #[test]
    fn renames_file() {
        let tmp = tempdir().unwrap();
        let source = tmp.path().join("old.rs");
        fs::write(&source, "content").unwrap();
        let target = rename_item(&source, "new.rs").unwrap();
        assert_eq!(fs::read_to_string(target).unwrap(), "content");
        assert!(!source.exists());
    }

    #[test]
    fn moves_without_overwriting_and_rejects_recursive_target() {
        let tmp = tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        fs::write(a.join("file.txt"), "new").unwrap();
        fs::write(b.join("file.txt"), "old").unwrap();
        let moved = move_into(&a.join("file.txt"), &b).unwrap();
        assert_eq!(fs::read_to_string(moved).unwrap(), "new");
        assert_eq!(fs::read_to_string(b.join("file.txt")).unwrap(), "old");
        let inside = b.join("inside");
        fs::create_dir(&inside).unwrap();
        assert_eq!(move_into(&b, &inside).unwrap_err().kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn export_does_not_overwrite_and_sanitizes_spreadsheet_formulas() {
        let tmp = tempdir().unwrap();
        let item = FileEntry {
            path: tmp.path().join("=SUM(A1)"),
            name: "=SUM(A1)".to_owned(),
            kind: EntryKind::File,
            size: 3,
            modified: None,
        };
        let first = export_csv(tmp.path(), std::slice::from_ref(&item)).unwrap();
        let second = export_csv(tmp.path(), &[item]).unwrap();
        assert_ne!(first, second);
        let contents = fs::read_to_string(first).unwrap();
        assert!(contents.contains("\"'=SUM(A1)\""));
    }
}
