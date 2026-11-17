//! The UltraViewer screen dictionary (`8bit.dat`), loaded at runtime.
//!
//! The file belongs to UltraViewer and has no redistribution license, so it is
//! never compiled into this binary or committed to this repository. Each user
//! supplies the copy from their own UltraViewer installation. Any candidate
//! file must match the pinned size and SHA-256 before it is used.

use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const DICTIONARY_FILE_NAME: &str = "8bit.dat";
pub const DICTIONARY_LEN: usize = 112_640;
pub const DICTIONARY_SHA256: &str =
    "9aa45789d9ff9d329a623a685d57b67e33c777019de9ce1f273f21860e4bfe6b";
/// Set this to a file path to skip the search and use that file only.
pub const DICTIONARY_ENV: &str = "ULTRAVIEWER_MCP_DICTIONARY";

const MACVIEWER_COPY: &str = "/Applications/MacViewer.app/Contents/Resources/8bit.dat";

#[derive(Clone, Debug, PartialEq)]
pub enum DictionaryError {
    NotFound { searched: Vec<PathBuf> },
    Invalid { path: PathBuf, reason: String },
    Io { path: PathBuf, reason: String },
}

impl fmt::Display for DictionaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DictionaryError::NotFound { searched } => {
                writeln!(
                    f,
                    "The UltraViewer screen dictionary ({DICTIONARY_FILE_NAME}) was not found. \
                     It is not distributed with ultraviewer-mcp. Get it from UltraViewer itself:"
                )?;
                writeln!(
                    f,
                    "  scripts/fetch-dictionary.sh   (downloads the official installer, extracts only 8bit.dat)"
                )?;
                writeln!(
                    f,
                    "  ultraviewer-mcp install-dictionary /path/to/8bit.dat   (copy from an UltraViewer install)"
                )?;
                write!(f, "Searched:")?;
                for path in searched {
                    write!(f, "\n  {}", path.display())?;
                }
                Ok(())
            }
            DictionaryError::Invalid { path, reason } => write!(
                f,
                "{} is not the expected UltraViewer screen dictionary: {reason}",
                path.display()
            ),
            DictionaryError::Io { path, reason } => {
                write!(f, "Could not read {}: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for DictionaryError {}

/// Checks the pinned size and SHA-256.
pub fn verify(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() != DICTIONARY_LEN {
        return Err(format!(
            "size is {} bytes, expected {DICTIONARY_LEN}",
            bytes.len()
        ));
    }
    let digest = hex(&Sha256::digest(bytes));
    if digest != DICTIONARY_SHA256 {
        return Err(format!("SHA-256 is {digest}, expected {DICTIONARY_SHA256}"));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Reads and verifies one file.
pub fn load_from(path: &Path) -> Result<Vec<u8>, DictionaryError> {
    let bytes = fs::read(path).map_err(|e| DictionaryError::Io {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })?;
    verify(&bytes).map_err(|reason| DictionaryError::Invalid {
        path: path.to_path_buf(),
        reason,
    })?;
    Ok(bytes)
}

/// Where `install-dictionary` puts the file.
pub fn installed_path() -> Option<PathBuf> {
    crate::profile::config_dir()
        .ok()
        .map(|dir| dir.join(DICTIONARY_FILE_NAME))
}

/// Search order when the environment variable is unset.
pub fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = installed_path() {
        paths.push(path);
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        paths.push(dir.join(DICTIONARY_FILE_NAME));
    }
    if cfg!(target_os = "macos") {
        paths.push(PathBuf::from(MACVIEWER_COPY));
    }
    paths
}

/// Finds a valid dictionary. An explicit environment path is used alone, so
/// a wrong path fails loudly instead of silently picking another copy.
pub fn locate() -> Result<(PathBuf, Vec<u8>), DictionaryError> {
    if let Some(path) = std::env::var_os(DICTIONARY_ENV).filter(|v| !v.is_empty()) {
        let path = PathBuf::from(path);
        let bytes = load_from(&path)?;
        return Ok((path, bytes));
    }
    let searched = candidate_paths();
    let mut first_invalid = None;
    for path in &searched {
        if !path.is_file() {
            continue;
        }
        match load_from(path) {
            Ok(bytes) => return Ok((path.clone(), bytes)),
            Err(error) => {
                first_invalid.get_or_insert(error);
            }
        }
    }
    Err(first_invalid.unwrap_or(DictionaryError::NotFound { searched }))
}

static LOADED: OnceLock<(PathBuf, Vec<u8>)> = OnceLock::new();

/// The verified dictionary. Only success is cached, so installing the file
/// while the server runs takes effect on the next connection.
pub fn screen_dictionary() -> Result<&'static [u8], DictionaryError> {
    loaded().map(|(_, bytes)| bytes.as_slice())
}

fn loaded() -> Result<&'static (PathBuf, Vec<u8>), DictionaryError> {
    if let Some(found) = LOADED.get() {
        return Ok(found);
    }
    let found = locate()?;
    Ok(LOADED.get_or_init(|| found))
}

/// One line for `session_status` and `check-dictionary`.
pub fn status_line() -> String {
    match loaded() {
        Ok((path, _)) => format!("Screen dictionary: verified at {}", path.display()),
        Err(DictionaryError::NotFound { .. }) => {
            "Screen dictionary: missing (run `ultraviewer-mcp install-dictionary <path>`)".into()
        }
        Err(error) => format!("Screen dictionary: unusable ({error})"),
    }
}

/// Verifies `source` and copies it to the config folder.
pub fn install(source: &Path) -> Result<PathBuf, DictionaryError> {
    let bytes = load_from(source)?;
    let target = installed_path().ok_or_else(|| DictionaryError::Io {
        path: PathBuf::from("~/.config/ultraviewer-mcp"),
        reason: "HOME is not set".into(),
    })?;
    let io = |path: &Path, e: std::io::Error| DictionaryError::Io {
        path: path.to_path_buf(),
        reason: e.to_string(),
    };
    let dir = target.parent().expect("config path has a parent");
    fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
    let tmp = target.with_extension("dat.tmp");
    fs::write(&tmp, &bytes).map_err(|e| io(&tmp, e))?;
    fs::rename(&tmp, &target).map_err(|e| io(&target, e))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_size_and_wrong_content() {
        assert!(verify(&[0u8; 16]).unwrap_err().contains("size is 16 bytes"));
        assert!(verify(&vec![0u8; DICTIONARY_LEN])
            .unwrap_err()
            .contains("SHA-256"));
    }

    #[test]
    fn missing_message_explains_how_to_install() {
        let text = DictionaryError::NotFound {
            searched: vec![PathBuf::from("/nowhere/8bit.dat")],
        }
        .to_string();
        assert!(text.contains("install-dictionary"));
        assert!(text.contains("/nowhere/8bit.dat"));
        assert!(text.contains("not distributed"));
    }

    #[test]
    fn invalid_file_is_reported_with_its_path() {
        let dir = std::env::temp_dir().join(format!("uvmcp-dict-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(DICTIONARY_FILE_NAME);
        fs::write(&path, b"not a dictionary").unwrap();
        let error = load_from(&path).unwrap_err();
        fs::remove_dir_all(&dir).unwrap();
        assert!(matches!(error, DictionaryError::Invalid { .. }));
        assert!(error.to_string().contains(&path.display().to_string()));
    }
}
