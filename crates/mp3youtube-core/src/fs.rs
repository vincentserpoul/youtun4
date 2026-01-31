//! File system abstraction for testability.
//!
//! This module provides a `FileSystem` trait that abstracts file system operations,
//! allowing for easy mocking in tests.
//!
//! # Example
//!
//! ```rust,ignore
//! use mp3youtube_core::fs::{FileSystem, RealFileSystem};
//!
//! fn read_config<F: FileSystem>(fs: &F, path: &Path) -> Result<String> {
//!     fs.read_to_string(path)
//! }
//!
//! // In production:
//! let fs = RealFileSystem;
//! let config = read_config(&fs, Path::new("config.json"))?;
//!
//! // In tests:
//! let mut mock = MockFileSystem::new();
//! mock.add_file("config.json", r#"{"key": "value"}"#);
//! let config = read_config(&mock, Path::new("config.json"))?;
//! ```

use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::{Error, FileSystemError, Result};

/// Converts an I/O error for read operations.
fn read_error(path: &Path, e: io::Error) -> Error {
    Error::FileSystem(FileSystemError::ReadFailed {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

/// Converts an I/O error for write operations.
fn write_error(path: &Path, e: io::Error) -> Error {
    Error::FileSystem(FileSystemError::WriteFailed {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

/// Converts an I/O error for directory creation.
fn create_dir_error(path: &Path, e: io::Error) -> Error {
    Error::FileSystem(FileSystemError::CreateDirFailed {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

/// Converts an I/O error for delete operations.
fn delete_error(path: &Path, e: io::Error) -> Error {
    Error::FileSystem(FileSystemError::DeleteFailed {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

/// Converts an I/O error for copy operations.
fn copy_error(src: &Path, dst: &Path, e: io::Error) -> Error {
    Error::FileSystem(FileSystemError::CopyFailed {
        source_path: src.to_path_buf(),
        destination: dst.to_path_buf(),
        reason: e.to_string(),
    })
}

/// Abstraction over file system operations for testability.
///
/// This trait allows components to be tested without touching the real file system.
pub trait FileSystem: Send + Sync {
    /// Read a file's contents as a string.
    fn read_to_string(&self, path: &Path) -> Result<String>;

    /// Read a file's contents as bytes.
    fn read(&self, path: &Path) -> Result<Vec<u8>>;

    /// Write string contents to a file, creating it if it doesn't exist.
    fn write(&self, path: &Path, contents: &str) -> Result<()>;

    /// Write bytes to a file, creating it if it doesn't exist.
    fn write_bytes(&self, path: &Path, contents: &[u8]) -> Result<()>;

    /// Check if a path exists.
    fn exists(&self, path: &Path) -> bool;

    /// Check if a path is a file.
    fn is_file(&self, path: &Path) -> bool;

    /// Check if a path is a directory.
    fn is_dir(&self, path: &Path) -> bool;

    /// Create a directory and all parent directories.
    fn create_dir_all(&self, path: &Path) -> Result<()>;

    /// Remove a file.
    fn remove_file(&self, path: &Path) -> Result<()>;

    /// Remove a directory and all its contents.
    fn remove_dir_all(&self, path: &Path) -> Result<()>;

    /// List entries in a directory.
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>>;

    /// Copy a file from src to dst.
    fn copy(&self, src: &Path, dst: &Path) -> Result<u64>;

    /// Rename/move a file or directory.
    fn rename(&self, from: &Path, to: &Path) -> Result<()>;

    /// Get file metadata (size, modified time, etc.).
    fn metadata(&self, path: &Path) -> Result<FileMetadata>;

    /// Get the canonical, absolute form of a path.
    fn canonicalize(&self, path: &Path) -> Result<PathBuf>;

    /// Walk a directory tree, returning all entries up to a given depth.
    ///
    /// # Arguments
    /// * `path` - The root directory to walk
    /// * `max_depth` - Maximum depth to recurse (1 = direct children only, None = unlimited)
    ///
    /// Returns a list of all file and directory paths found.
    fn walk_dir(&self, path: &Path, max_depth: Option<usize>) -> Result<Vec<PathBuf>>;
}

/// Simplified metadata structure for cross-platform compatibility.
#[derive(Debug, Clone)]
pub struct FileMetadata {
    /// File size in bytes.
    pub len: u64,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Whether this is a file.
    pub is_file: bool,
    /// Last modified time.
    pub modified: Option<SystemTime>,
}

impl FileMetadata {
    /// Create metadata from std::fs::Metadata.
    pub fn from_std(meta: Metadata) -> Self {
        Self {
            len: meta.len(),
            is_dir: meta.is_dir(),
            is_file: meta.is_file(),
            modified: meta.modified().ok(),
        }
    }
}

/// Real file system implementation using std::fs.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealFileSystem;

impl RealFileSystem {
    /// Create a new real file system instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl FileSystem for RealFileSystem {
    fn read_to_string(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path).map_err(|e| read_error(path, e))
    }

    fn read(&self, path: &Path) -> Result<Vec<u8>> {
        fs::read(path).map_err(|e| read_error(path, e))
    }

    fn write(&self, path: &Path, contents: &str) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|e| create_dir_error(parent, e))?;
        }
        fs::write(path, contents).map_err(|e| write_error(path, e))
    }

    fn write_bytes(&self, path: &Path, contents: &[u8]) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|e| create_dir_error(parent, e))?;
        }
        fs::write(path, contents).map_err(|e| write_error(path, e))
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path).map_err(|e| create_dir_error(path, e))
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        fs::remove_file(path).map_err(|e| delete_error(path, e))
    }

    fn remove_dir_all(&self, path: &Path) -> Result<()> {
        fs::remove_dir_all(path).map_err(|e| delete_error(path, e))
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let entries = fs::read_dir(path).map_err(|e| read_error(path, e))?;

        let paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        Ok(paths)
    }

    fn copy(&self, src: &Path, dst: &Path) -> Result<u64> {
        // Ensure parent directory exists
        if let Some(parent) = dst.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|e| create_dir_error(parent, e))?;
        }
        fs::copy(src, dst).map_err(|e| copy_error(src, dst, e))
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        fs::rename(from, to).map_err(|e| copy_error(from, to, e))
    }

    fn metadata(&self, path: &Path) -> Result<FileMetadata> {
        let meta = fs::metadata(path).map_err(|e| read_error(path, e))?;
        Ok(FileMetadata::from_std(meta))
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf> {
        fs::canonicalize(path).map_err(|e| read_error(path, e))
    }

    fn walk_dir(&self, path: &Path, max_depth: Option<usize>) -> Result<Vec<PathBuf>> {
        fn walk_recursive(
            dir: &Path,
            current_depth: usize,
            max_depth: Option<usize>,
            results: &mut Vec<PathBuf>,
        ) -> io::Result<()> {
            if let Some(max) = max_depth
                && current_depth > max
            {
                return Ok(());
            }

            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                results.push(path.clone());

                if path.is_dir() {
                    walk_recursive(&path, current_depth + 1, max_depth, results)?;
                }
            }
            Ok(())
        }

        let mut results = Vec::new();
        walk_recursive(path, 1, max_depth, &mut results).map_err(|e| read_error(path, e))?;
        Ok(results)
    }
}

#[cfg(test)]
pub mod mock {
    //! Mock file system for testing.

    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};

    /// In-memory mock file system for testing.
    #[derive(Debug, Clone, Default)]
    pub struct MockFileSystem {
        files: Arc<RwLock<HashMap<PathBuf, Vec<u8>>>>,
        dirs: Arc<RwLock<std::collections::HashSet<PathBuf>>>,
    }

    impl MockFileSystem {
        /// Create a new empty mock file system.
        #[must_use]
        pub fn new() -> Self {
            Self {
                files: Arc::new(RwLock::new(HashMap::new())),
                dirs: Arc::new(RwLock::new(std::collections::HashSet::new())),
            }
        }

        /// Add a file with string contents.
        pub fn add_file(&self, path: impl AsRef<Path>, contents: &str) {
            let path = path.as_ref().to_path_buf();
            // Add parent directories
            if let Some(parent) = path.parent() {
                self.add_dir(parent);
            }
            self.files
                .write()
                .expect("lock poisoned")
                .insert(path, contents.as_bytes().to_vec());
        }

        /// Add a file with byte contents.
        pub fn add_file_bytes(&self, path: impl AsRef<Path>, contents: &[u8]) {
            let path = path.as_ref().to_path_buf();
            if let Some(parent) = path.parent() {
                self.add_dir(parent);
            }
            self.files
                .write()
                .expect("lock poisoned")
                .insert(path, contents.to_vec());
        }

        /// Add a directory.
        pub fn add_dir(&self, path: impl AsRef<Path>) {
            let path = path.as_ref();
            let mut dirs = self.dirs.write().expect("lock poisoned");

            // Add all parent directories too
            let mut current = path.to_path_buf();
            while current.parent().is_some() {
                dirs.insert(current.clone());
                if let Some(parent) = current.parent() {
                    current = parent.to_path_buf();
                } else {
                    break;
                }
            }
        }

        /// Get all files in the mock filesystem.
        #[must_use]
        pub fn list_all_files(&self) -> Vec<PathBuf> {
            self.files
                .read()
                .expect("lock poisoned")
                .keys()
                .cloned()
                .collect()
        }
    }

    impl FileSystem for MockFileSystem {
        fn read_to_string(&self, path: &Path) -> Result<String> {
            let files = self.files.read().expect("lock poisoned");
            files
                .get(path)
                .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
                .ok_or_else(|| {
                    Error::FileSystem(FileSystemError::NotFound {
                        path: path.to_path_buf(),
                    })
                })
        }

        fn read(&self, path: &Path) -> Result<Vec<u8>> {
            let files = self.files.read().expect("lock poisoned");
            files.get(path).cloned().ok_or_else(|| {
                Error::FileSystem(FileSystemError::NotFound {
                    path: path.to_path_buf(),
                })
            })
        }

        fn write(&self, path: &Path, contents: &str) -> Result<()> {
            self.add_file(path, contents);
            Ok(())
        }

        fn write_bytes(&self, path: &Path, contents: &[u8]) -> Result<()> {
            self.add_file_bytes(path, contents);
            Ok(())
        }

        fn exists(&self, path: &Path) -> bool {
            let files = self.files.read().expect("lock poisoned");
            let dirs = self.dirs.read().expect("lock poisoned");
            files.contains_key(path) || dirs.contains(path)
        }

        fn is_file(&self, path: &Path) -> bool {
            let files = self.files.read().expect("lock poisoned");
            files.contains_key(path)
        }

        fn is_dir(&self, path: &Path) -> bool {
            let dirs = self.dirs.read().expect("lock poisoned");
            dirs.contains(path)
        }

        fn create_dir_all(&self, path: &Path) -> Result<()> {
            self.add_dir(path);
            Ok(())
        }

        fn remove_file(&self, path: &Path) -> Result<()> {
            let mut files = self.files.write().expect("lock poisoned");
            files.remove(path);
            Ok(())
        }

        fn remove_dir_all(&self, path: &Path) -> Result<()> {
            let mut files = self.files.write().expect("lock poisoned");
            let mut dirs = self.dirs.write().expect("lock poisoned");

            // Remove all files under this path
            files.retain(|p, _| !p.starts_with(path));
            // Remove all dirs under this path
            dirs.retain(|p| !p.starts_with(path));

            Ok(())
        }

        fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
            let files = self.files.read().expect("lock poisoned");
            let dirs = self.dirs.read().expect("lock poisoned");

            let mut entries = std::collections::HashSet::new();

            // Find direct children (files)
            for file_path in files.keys() {
                if let Some(parent) = file_path.parent() {
                    if parent == path {
                        entries.insert(file_path.clone());
                    }
                }
            }

            // Find direct children (dirs)
            for dir_path in dirs.iter() {
                if let Some(parent) = dir_path.parent() {
                    if parent == path && dir_path != path {
                        entries.insert(dir_path.clone());
                    }
                }
            }

            Ok(entries.into_iter().collect())
        }

        fn copy(&self, src: &Path, dst: &Path) -> Result<u64> {
            let contents = self.read(src)?;
            let len = contents.len() as u64;
            self.write_bytes(dst, &contents)?;
            Ok(len)
        }

        fn rename(&self, from: &Path, to: &Path) -> Result<()> {
            let contents = self.read(from)?;
            self.write_bytes(to, &contents)?;
            self.remove_file(from)?;
            Ok(())
        }

        fn metadata(&self, path: &Path) -> Result<FileMetadata> {
            let files = self.files.read().expect("lock poisoned");
            let dirs = self.dirs.read().expect("lock poisoned");

            if let Some(contents) = files.get(path) {
                Ok(FileMetadata {
                    len: contents.len() as u64,
                    is_dir: false,
                    is_file: true,
                    modified: Some(SystemTime::now()),
                })
            } else if dirs.contains(path) {
                Ok(FileMetadata {
                    len: 0,
                    is_dir: true,
                    is_file: false,
                    modified: Some(SystemTime::now()),
                })
            } else {
                Err(Error::FileSystem(FileSystemError::NotFound {
                    path: path.to_path_buf(),
                }))
            }
        }

        fn canonicalize(&self, path: &Path) -> Result<PathBuf> {
            // In mock, just return the path as-is (normalized)
            Ok(path.to_path_buf())
        }

        fn walk_dir(&self, path: &Path, max_depth: Option<usize>) -> Result<Vec<PathBuf>> {
            fn collect_entries(
                fs: &MockFileSystem,
                dir: &Path,
                current_depth: usize,
                max_depth: Option<usize>,
                results: &mut Vec<PathBuf>,
            ) {
                if let Some(max) = max_depth
                    && current_depth > max
                {
                    return;
                }

                // Get direct children
                if let Ok(entries) = fs.read_dir(dir) {
                    for entry in entries {
                        results.push(entry.clone());
                        if fs.is_dir(&entry) {
                            collect_entries(fs, &entry, current_depth + 1, max_depth, results);
                        }
                    }
                }
            }

            let mut results = Vec::new();
            collect_entries(self, path, 1, max_depth, &mut results);
            Ok(results)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::MockFileSystem;
    use super::*;

    #[test]
    fn test_mock_fs_read_write() {
        let fs = MockFileSystem::new();

        fs.write(Path::new("/test/file.txt"), "hello world")
            .unwrap();

        let contents = fs.read_to_string(Path::new("/test/file.txt")).unwrap();
        assert_eq!(contents, "hello world");
    }

    #[test]
    fn test_mock_fs_exists() {
        let fs = MockFileSystem::new();

        assert!(!fs.exists(Path::new("/test/file.txt")));

        fs.write(Path::new("/test/file.txt"), "hello").unwrap();

        assert!(fs.exists(Path::new("/test/file.txt")));
        assert!(fs.exists(Path::new("/test"))); // Parent dir created
    }

    #[test]
    fn test_mock_fs_is_file_is_dir() {
        let fs = MockFileSystem::new();

        fs.write(Path::new("/test/file.txt"), "hello").unwrap();
        fs.create_dir_all(Path::new("/test/subdir")).unwrap();

        assert!(fs.is_file(Path::new("/test/file.txt")));
        assert!(!fs.is_dir(Path::new("/test/file.txt")));

        assert!(fs.is_dir(Path::new("/test/subdir")));
        assert!(!fs.is_file(Path::new("/test/subdir")));
    }

    #[test]
    fn test_mock_fs_remove() {
        let fs = MockFileSystem::new();

        fs.write(Path::new("/test/file.txt"), "hello").unwrap();
        assert!(fs.exists(Path::new("/test/file.txt")));

        fs.remove_file(Path::new("/test/file.txt")).unwrap();
        assert!(!fs.exists(Path::new("/test/file.txt")));
    }

    #[test]
    fn test_mock_fs_read_dir() {
        let fs = MockFileSystem::new();

        fs.write(Path::new("/test/a.txt"), "a").unwrap();
        fs.write(Path::new("/test/b.txt"), "b").unwrap();
        fs.create_dir_all(Path::new("/test/subdir")).unwrap();

        let entries = fs.read_dir(Path::new("/test")).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_mock_fs_copy() {
        let fs = MockFileSystem::new();

        fs.write(Path::new("/src/file.txt"), "content").unwrap();

        let bytes = fs
            .copy(Path::new("/src/file.txt"), Path::new("/dst/file.txt"))
            .unwrap();

        assert_eq!(bytes, 7);
        assert!(fs.exists(Path::new("/dst/file.txt")));
        assert_eq!(
            fs.read_to_string(Path::new("/dst/file.txt")).unwrap(),
            "content"
        );
    }

    #[test]
    fn test_mock_fs_metadata() {
        let fs = MockFileSystem::new();

        fs.write(Path::new("/test/file.txt"), "hello").unwrap();

        let meta = fs.metadata(Path::new("/test/file.txt")).unwrap();
        assert_eq!(meta.len, 5);
        assert!(meta.is_file);
        assert!(!meta.is_dir);
    }

    #[test]
    fn test_real_fs_basic() {
        // Quick sanity test for RealFileSystem
        let fs = RealFileSystem::new();

        // Current directory should exist
        assert!(fs.exists(Path::new(".")));
        assert!(fs.is_dir(Path::new(".")));
    }
}
