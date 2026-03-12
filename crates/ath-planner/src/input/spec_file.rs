//! Spec file reader with size cap enforcement.
//!
//! Reads markdown spec files and enforces a 50KB size limit to prevent
//! accidental cost explosions from large files being sent to LLMs.

use std::path::Path;

use super::error::InputError;

/// Maximum spec file size: 50KB (51,200 bytes).
pub const MAX_SPEC_FILE_SIZE: u64 = 51_200;

/// Read a spec file, enforcing the size cap before reading contents.
///
/// Returns `SpecFileNotFound` if the file does not exist.
/// Returns `SpecFileTooLarge` if the file exceeds `MAX_SPEC_FILE_SIZE`.
pub async fn read_spec_file(path: &Path) -> Result<String, InputError> {
    // Check file exists
    let metadata = tokio::fs::metadata(path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            InputError::SpecFileNotFound {
                path: path.to_path_buf(),
                hint: "Check the file path".into(),
            }
        } else {
            InputError::Io(e)
        }
    })?;

    // Check size before reading
    let size = metadata.len();
    if size > MAX_SPEC_FILE_SIZE {
        return Err(InputError::SpecFileTooLarge {
            size,
            max: MAX_SPEC_FILE_SIZE,
            hint: format!(
                "Spec files must be under {} bytes (50KB). Consider splitting the document.",
                MAX_SPEC_FILE_SIZE
            ),
        });
    }

    // Read contents
    let content = tokio::fs::read_to_string(path).await.map_err(InputError::Io)?;

    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn read_spec_file_under_limit_succeeds() {
        let mut file = NamedTempFile::new().expect("create temp file");
        write!(file, "# My Project\n\nA cool project spec.").expect("write");
        file.flush().expect("flush");

        let result = read_spec_file(file.path()).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("My Project"));
    }

    #[tokio::test]
    async fn read_spec_file_over_limit_returns_too_large() {
        let mut file = NamedTempFile::new().expect("create temp file");
        // Write more than 50KB
        let data = "x".repeat(MAX_SPEC_FILE_SIZE as usize + 1);
        write!(file, "{}", data).expect("write");
        file.flush().expect("flush");

        let result = read_spec_file(file.path()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            InputError::SpecFileTooLarge { size, max, .. } => {
                assert!(size > MAX_SPEC_FILE_SIZE);
                assert_eq!(max, MAX_SPEC_FILE_SIZE);
            }
            other => panic!("expected SpecFileTooLarge, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn read_spec_file_not_found_returns_error() {
        let path = Path::new("/tmp/definitely-does-not-exist-athena-test.md");
        let result = read_spec_file(path).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            InputError::SpecFileNotFound { path: p, .. } => {
                assert_eq!(p, path);
            }
            other => panic!("expected SpecFileNotFound, got: {:?}", other),
        }
    }
}
