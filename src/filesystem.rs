//! Filesystem operations deliberately separated from the graphical interface.
//! Read and copy operations do not follow symbolic links.

use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    time::SystemTime,
};

use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Name,
    Size,
    Modified,
}

impl SortBy {
    pub fn next(self) -> Self {
        match self {
            Self::Name => Self::Size,
            Self::Size => Self::Modified,
            Self::Modified => Self::Name,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Nome",
            Self::Size => "Tamanho",
            Self::Modified => "Modificado",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Folder,
    File,
    Symlink,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub kind: EntryKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

impl FileEntry {
    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            EntryKind::Folder => "Pasta",
            EntryKind::File => "Arquivo",
            EntryKind::Symlink => "Atalho / link",
        }
    }
}

/// Scan a single directory. No recursion and no symlink traversal.
pub fn read_directory(path: &Path, show_hidden: bool, sort: SortBy) -> io::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();
    for result in fs::read_dir(path)? {
        let entry = result?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            // Files can disappear between read_dir and metadata: skip these races.
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if !show_hidden && is_hidden(&name, &metadata) {
            continue;
        }
        let kind = if metadata.file_type().is_symlink() {
            EntryKind::Symlink
        } else if metadata.is_dir() {
            EntryKind::Folder
        } else {
            EntryKind::File
        };
        entries.push(FileEntry {
            path: entry.path(),
            name,
            kind,
            size: metadata.len(),
            modified: metadata.modified().ok(),
        });
    }

    match sort {
        SortBy::Name => {
            // Cache case-insensitive keys once per entry, not on every comparison.
            entries.sort_by_cached_key(|entry| {
                (
                    u8::from(entry.kind != EntryKind::Folder),
                    entry.name.to_lowercase(),
                    entry.name.clone(),
                )
            });
        }
        SortBy::Size => {
            entries.sort_by(|a, b| {
                u8::from(a.kind != EntryKind::Folder)
                    .cmp(&u8::from(b.kind != EntryKind::Folder))
                    .then_with(|| a.size.cmp(&b.size))
                    .then_with(|| a.name.cmp(&b.name))
            });
        }
        SortBy::Modified => {
            entries.sort_by(|a, b| {
                u8::from(a.kind != EntryKind::Folder)
                    .cmp(&u8::from(b.kind != EntryKind::Folder))
                    .then_with(|| a.modified.cmp(&b.modified))
                    .then_with(|| a.name.cmp(&b.name))
            });
        }
    }
    Ok(entries)
}

fn is_hidden(name: &str, metadata: &fs::Metadata) -> bool {
    if name.starts_with('.') {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

/// Create a new folder without overwriting an existing entry.
pub fn create_folder(parent: &Path) -> io::Result<PathBuf> {
    for index in 1..=10_000 {
        let name = if index == 1 {
            "Nova pasta".to_string()
        } else {
            format!("Nova pasta ({index})")
        };
        let target = parent.join(name);
        match fs::create_dir(&target) {
            Ok(()) => return Ok(target),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        ErrorKind::AlreadyExists,
        "Muitas pastas com o mesmo nome",
    ))
}

fn is_available(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
}

/// Resolve copy collisions without silently replacing anything.
pub fn unique_copy_path(
    parent: &Path,
    original_name: &OsStr,
    directory: bool,
) -> io::Result<PathBuf> {
    let original = Path::new(original_name);
    let first = parent.join(original);
    if is_available(&first)? {
        return Ok(first);
    }
    let stem = if directory {
        original_name.to_string_lossy().into_owned()
    } else {
        original
            .file_stem()
            .unwrap_or(original_name)
            .to_string_lossy()
            .into_owned()
    };
    let extension = if directory {
        String::new()
    } else {
        original
            .extension()
            .map(|value| format!(".{}", value.to_string_lossy()))
            .unwrap_or_default()
    };
    for index in 1..=10_000 {
        let suffix = if index == 1 {
            " (cópia)".to_string()
        } else {
            format!(" (cópia {index})")
        };
        let target = parent.join(format!("{stem}{suffix}{extension}"));
        if is_available(&target)? {
            return Ok(target);
        }
    }
    Err(io::Error::new(
        ErrorKind::AlreadyExists,
        "Muitas cópias do mesmo arquivo",
    ))
}

/// Copy a file or tree to an existing directory.
///
/// We preflight the source and refuse any symlinks; nested targets are rejected.
/// Every regular file is opened with create_new, preventing silent replacement.
pub fn copy_into(source: &Path, destination_directory: &Path) -> io::Result<PathBuf> {
    if !destination_directory.is_dir() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "O destino não é uma pasta",
        ));
    }
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "Não copiamos links simbólicos",
        ));
    }
    let name = source
        .file_name()
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "Caminho de origem inválido"))?;

    if metadata.is_dir() {
        let canonical_source = fs::canonicalize(source)?;
        let canonical_destination = fs::canonicalize(destination_directory)?;
        if canonical_destination.starts_with(&canonical_source) {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "Não copie uma pasta para dentro dela mesma",
            ));
        }

        // Preflight, so unsupported links fail before writing anything.
        let mut paths = Vec::new();
        for item in WalkDir::new(source).follow_links(false) {
            let item = item.map_err(io::Error::other)?;
            if item.file_type().is_symlink() {
                return Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    "A pasta contém links simbólicos",
                ));
            }
            paths.push((item.path().to_path_buf(), item.file_type().is_dir()));
        }

        let target = unique_copy_path(destination_directory, name, true)?;
        fs::create_dir(&target)?;
        for (original, is_dir) in paths.into_iter().skip(1) {
            let relative = original.strip_prefix(source).map_err(io::Error::other)?;
            let destination = target.join(relative);
            if is_dir {
                fs::create_dir(&destination)?;
            } else {
                copy_new_file(&original, &destination)?;
            }
        }
        Ok(target)
    } else {
        let target = unique_copy_path(destination_directory, name, false)?;
        copy_new_file(source, &target)?;
        Ok(target)
    }
}

fn copy_new_file(source: &Path, target: &Path) -> io::Result<()> {
    let mut reader = File::open(source)?;
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    io::copy(&mut reader, &mut writer)?;
    Ok(())
}

pub fn move_to_trash(path: &Path) -> Result<(), trash::Error> {
    trash::delete(path)
}

pub fn display_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut number = bytes as f64;
    let mut unit = 0;
    while number >= 1024.0 && unit < UNITS.len() - 1 {
        number /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{number:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn lists_directories_first_and_can_hide_dotfiles() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("z.txt"), "hi").unwrap();
        fs::write(dir.path().join(".secret"), "hidden").unwrap();
        fs::create_dir(dir.path().join("alpha")).unwrap();
        let entries = read_directory(dir.path(), false, SortBy::Name).unwrap();
        assert_eq!(
            entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            vec!["alpha", "z.txt"]
        );
        assert_eq!(
            read_directory(dir.path(), true, SortBy::Name)
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn creates_folders_without_clobbering() {
        let dir = tempdir().unwrap();
        let first = create_folder(dir.path()).unwrap();
        let second = create_folder(dir.path()).unwrap();
        assert_ne!(first, second);
        assert!(first.is_dir() && second.is_dir());
    }

    #[test]
    fn copy_renames_conflicts_and_preserves_content() {
        let root = tempdir().unwrap();
        let destination = root.path().join("out");
        fs::create_dir(&destination).unwrap();
        let source = root.path().join("file.txt");
        fs::write(&source, "hello").unwrap();
        let first = copy_into(&source, &destination).unwrap();
        let second = copy_into(&source, &destination).unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read_to_string(second).unwrap(), "hello");
    }

    #[test]
    fn copy_keeps_existing_destination_unchanged() {
        let root = tempdir().unwrap();
        let out = root.path().join("out");
        fs::create_dir(&out).unwrap();
        let source = root.path().join("file.txt");
        fs::write(&source, "new").unwrap();
        fs::write(out.join("file.txt"), "existing").unwrap();
        let copied = copy_into(&source, &out).unwrap();
        assert_ne!(copied, out.join("file.txt"));
        assert_eq!(
            fs::read_to_string(out.join("file.txt")).unwrap(),
            "existing"
        );
        assert_eq!(fs::read_to_string(copied).unwrap(), "new");
    }

    #[test]
    fn copy_rejects_missing_target_directory() {
        let root = tempdir().unwrap();
        let source = root.path().join("file.txt");
        fs::write(&source, "test").unwrap();
        assert_eq!(
            copy_into(&source, &root.path().join("missing"))
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidInput,
        );
    }

    #[cfg(unix)]
    #[test]
    fn copy_rejects_symlink_sources() {
        use std::os::unix::fs::symlink;
        let root = tempdir().unwrap();
        let source = root.path().join("source.txt");
        fs::write(&source, "test").unwrap();
        let link = root.path().join("link.txt");
        symlink(&source, &link).unwrap();
        let out = root.path().join("out");
        fs::create_dir(&out).unwrap();
        assert_eq!(
            copy_into(&link, &out).unwrap_err().kind(),
            ErrorKind::InvalidInput,
        );
    }

    #[test]
    fn rejects_recursive_copy_into_child() {
        let root = tempdir().unwrap();
        let sub = root.path().join("inner");
        fs::create_dir(&sub).unwrap();
        assert_eq!(
            copy_into(root.path(), &sub).unwrap_err().kind(),
            ErrorKind::InvalidInput
        );
    }

    #[test]
    fn recursively_copies_directories() {
        let root = tempdir().unwrap();
        let source = root.path().join("source");
        let nested = source.join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("data.bin"), [1u8, 2, 3]).unwrap();
        let out = root.path().join("out");
        fs::create_dir(&out).unwrap();
        let copied = copy_into(&source, &out).unwrap();
        assert_eq!(
            fs::read(copied.join("nested/data.bin")).unwrap(),
            vec![1, 2, 3]
        );
    }
}
