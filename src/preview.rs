//! Bounded, read-only previews for local files.
//!
//! Parsing is deliberately conservative: UTF-8 text and common raster images.
//! No third-party PDF/Office parsers or external processes execute here.

use std::{
    fs::{self, File},
    io::{self, ErrorKind, Read},
    path::{Path, PathBuf},
};

/// Maximum text bytes displayed in the preview panel.
pub const TEXT_LIMIT: u64 = 64 * 1024;
/// Above this size we do not ask GPUI to decode an image automatically.
pub const MAX_IMAGE_BYTES: u64 = 24 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewData {
    Text { content: String, truncated: bool },
    Image(PathBuf),
    Unsupported(String),
}

pub fn read_preview(path: &Path) -> io::Result<PreviewData> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(PreviewData::Unsupported(
            "A pré-visualização não abre pastas nem links simbólicos.".to_owned(),
        ));
    }
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
    ) {
        return if metadata.len() <= MAX_IMAGE_BYTES {
            Ok(PreviewData::Image(path.to_path_buf()))
        } else {
            Ok(PreviewData::Unsupported(format!(
                "Imagem muito grande para pré-visualizar (limite: {} MB).",
                MAX_IMAGE_BYTES / 1024 / 1024
            )))
        };
    }

    if !matches!(
        extension.as_str(),
        "txt" | "md" | "markdown" | "rs" | "toml" | "json" | "yaml" | "yml"
            | "xml" | "html" | "htm" | "css" | "js" | "ts" | "jsx" | "tsx"
            | "py" | "go" | "java" | "c" | "h" | "cpp" | "hpp" | "cs"
            | "sql" | "log" | "ini" | "cfg" | "csv" | "bat" | "ps1" | "sh"
    ) {
        return Ok(PreviewData::Unsupported(
            "Não há pré-visualizador para esse formato.".to_owned(),
        ));
    }

    let mut file = File::open(path)?;
    // At most TEXT_LIMIT bytes are ever held in memory for text previews.
    let mut data = Vec::new();
    file.by_ref().take(TEXT_LIMIT).read_to_end(&mut data)?;
    if data.iter().any(|byte| *byte == 0) {
        return Ok(PreviewData::Unsupported(
            "Este arquivo parece ser binário.".to_owned(),
        ));
    }
    let content = String::from_utf8(data).map_err(|_| {
        io::Error::new(ErrorKind::InvalidData, "Texto não codificado em UTF-8")
    })?;
    Ok(PreviewData::Text {
        content,
        truncated: metadata.len() > TEXT_LIMIT,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn reads_utf8_text() {
        let tmp = tempdir().unwrap();
        let source = tmp.path().join("exemplo.rs");
        fs::write(&source, "fn main() {}\n").unwrap();
        assert_eq!(
            read_preview(&source).unwrap(),
            PreviewData::Text {
                content: "fn main() {}\n".to_owned(),
                truncated: false,
            }
        );
    }

    #[test]
    fn limits_text_and_marks_truncation() {
        let tmp = tempdir().unwrap();
        let source = tmp.path().join("grande.txt");
        fs::write(&source, vec![b'a'; TEXT_LIMIT as usize + 25]).unwrap();
        match read_preview(&source).unwrap() {
            PreviewData::Text { content, truncated } => {
                assert_eq!(content.len(), TEXT_LIMIT as usize);
                assert!(truncated);
            }
            other => panic!("Unexpected preview: {other:?}"),
        }
    }

    #[test]
    fn rejects_binary_and_unknown_formats() {
        let tmp = tempdir().unwrap();
        let binary = tmp.path().join("bin.txt");
        fs::write(&binary, [0, 1, 2]).unwrap();
        assert!(matches!(
            read_preview(&binary).unwrap(),
            PreviewData::Unsupported(_)
        ));
        let unknown = tmp.path().join("movie.mkv");
        fs::write(&unknown, b"data").unwrap();
        assert!(matches!(
            read_preview(&unknown).unwrap(),
            PreviewData::Unsupported(_)
        ));
    }

    #[test]
    fn avoids_loading_oversized_images() {
        let tmp = tempdir().unwrap();
        let image = tmp.path().join("picture.png");
        let file = File::create(&image).unwrap();
        file.set_len(MAX_IMAGE_BYTES + 1).unwrap();
        assert!(matches!(
            read_preview(&image).unwrap(),
            PreviewData::Unsupported(_)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_symlinks() {
        use std::os::unix::fs::symlink;
        let tmp = tempdir().unwrap();
        let data = tmp.path().join("actual.txt");
        fs::write(&data, "test").unwrap();
        let link = tmp.path().join("link.txt");
        symlink(&data, &link).unwrap();
        assert!(matches!(
            read_preview(&link).unwrap(),
            PreviewData::Unsupported(_)
        ));
    }
}
