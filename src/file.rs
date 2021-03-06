use std::path::PathBuf;
use std::cmp::Ordering;
use std::sync::Mutex;
use std::io;
use metadata::Metadata;
use hasher::Hasher;

#[derive(Debug, Clone)]
pub struct FileSet {
    pub paths: Vec<PathBuf>
}

impl FileSet {
    pub fn new(path: PathBuf) -> Self {
        FileSet {
            paths: vec![path],
        }
    }

    pub fn push(&mut self, path: PathBuf) {
        self.paths.push(path);
    }
}


#[derive(Debug)]
/// File content is efficiently compared using this struct's PartialOrd implementation
pub struct FileContent {
    path: PathBuf,
    metadata: Metadata,
    /// Hashes of content, calculated incrementally
    hashes: Mutex<Hasher>,
}

impl FileContent {
    pub fn from_path<P: Into<PathBuf>>(path: P) -> Result<Self, io::Error> {
        let path = path.into();
        let m = Metadata::from_path(&path)?;
        Ok(Self::new(path, m))
    }

    pub fn new<P: Into<PathBuf>>(path: P, metadata: Metadata) -> Self {
        let path = path.into();
        FileContent {
            path: path,
            metadata: metadata,
            hashes: Mutex::new(Hasher::new()),
        }
    }
}

impl Eq for FileContent {
}

impl PartialEq for FileContent {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other).map(|o|o == Ordering::Equal).unwrap_or(false)
    }
}

impl Ord for FileContent {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).expect("Error handling here sucks")
    }
}

/// That does the bulk of hasing and comparisons
impl PartialOrd for FileContent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Different file sizes mean they're obviously different.
        // Also different devices mean they're not the same as far as we're concerned
        // (since search is intended for hardlinking and hardlinking only works within the same device).
        let cmp = self.metadata.cmp(&other.metadata);
        if cmp != Ordering::Equal {
            return Some(cmp);
        }

        // Fast pointer comparison
        if self as *const _ == other as *const _ {
            return Some(Ordering::Equal);
        }

        let mut hashes1 = self.hashes.lock().unwrap();
        let mut hashes2 = other.hashes.lock().unwrap();

        hashes1.compare(&mut *hashes2, self.metadata.size, &self.path, &other.path).ok()
    }
}

#[test]
fn different_files() {
    let a = FileContent::from_path("tests/a").unwrap();
    let b = FileContent::from_path("tests/b").unwrap();
    assert_eq!(a, a);
    assert_eq!(b, b);
    assert_ne!(a, b);
}

#[test]
fn same_content() {
    let a = FileContent::from_path("tests/a").unwrap();
    let b = FileContent::from_path("tests/a2").unwrap();
    assert_eq!(a, a);
    assert_eq!(b, b);
    assert_eq!(a, b);
}

#[test]
fn symlink() {
    let a = FileContent::from_path("tests/a").unwrap();
    let c = FileContent::from_path("tests/c").unwrap();
    assert_ne!(a, c);
    assert_eq!(c, c);
}

#[test]
fn hardlink_of_same_file() {
    use std::io::Write;
    use std::fs;
    use tempdir::TempDir;

    let dir = TempDir::new("hardlinktest").unwrap();
    let a_path = dir.path().join("a");
    let b_path = dir.path().join("b");

    let mut a_fd = fs::File::create(&a_path).unwrap();
    a_fd.write_all(b"hello").unwrap();
    drop(a_fd);

    fs::hard_link(&a_path, &b_path).unwrap();

    let a = FileContent::from_path(a_path).unwrap();
    let b = FileContent::from_path(b_path).unwrap();
    assert_eq!(a, b);
    assert_eq!(b, b);
}
