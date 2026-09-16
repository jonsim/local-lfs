//! Raw, content-addressed object storage for the first Git LFS milestone.
//! An object is published only after its complete byte count and SHA-256 hash
//! match the values claimed by the upload request.

use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum StoreError {
    InvalidOid,
    SizeMismatch { expected: u64, actual: u64 },
    HashMismatch,
    NotFound,
    Read(io::Error),
    Io(io::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::InvalidOid => write!(f, "object ID must be 64 lowercase hex characters"),
            StoreError::SizeMismatch { expected, actual } => {
                write!(f, "expected {} object bytes, received {}", expected, actual)
            }
            StoreError::HashMismatch => write!(f, "object SHA-256 does not match its ID"),
            StoreError::NotFound => write!(f, "object does not exist"),
            StoreError::Read(error) => write!(f, "failed to read upload: {}", error),
            StoreError::Io(error) => write!(f, "object store I/O error: {}", error),
        }
    }
}

pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    /// Create the configured store directory if it does not already exist.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        fs::create_dir_all(root.join("objects")).map_err(StoreError::Io)?;
        Ok(Self { root })
    }

    /// Read exactly `size` bytes, hash them as they arrive, and publish the
    /// object only after both the count and the claimed ID have been checked.
    /// The reader is expected to be framed by the caller's HTTP request.
    pub fn put<R: Read>(&self, oid: &str, size: u64, reader: &mut R) -> Result<(), StoreError> {
        let path = self.object_path(oid)?;
        let directory = path.parent().expect("object path has a parent");
        fs::create_dir_all(directory).map_err(StoreError::Io)?;
        let mut temp = TempObject::create(directory)?;
        let mut hash = Sha256::new();
        let mut received = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];

        while received < size {
            let remaining = (size - received).min(buffer.len() as u64) as usize;
            let count = reader
                .read(&mut buffer[..remaining])
                .map_err(StoreError::Read)?;
            if count == 0 {
                return Err(StoreError::SizeMismatch {
                    expected: size,
                    actual: received,
                });
            }
            temp.file_mut()
                .write_all(&buffer[..count])
                .map_err(StoreError::Io)?;
            hash.update(&buffer[..count]);
            received += count as u64;
        }

        // Lowercase hex is the canonical form required by object_path.
        if hex::encode(hash.finalize()) != oid {
            return Err(StoreError::HashMismatch);
        }
        temp.commit(&path)
    }

    /// Open a verified-name object for streaming to an HTTP response.
    pub fn open(&self, oid: &str) -> Result<(File, u64), StoreError> {
        let path = self.object_path(oid)?;
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(StoreError::NotFound)
            }
            Err(error) => return Err(StoreError::Io(error)),
        };
        let metadata = file.metadata().map_err(StoreError::Io)?;
        if !metadata.is_file() {
            return Err(StoreError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "object path is not a regular file",
            )));
        }
        let size = metadata.len();
        Ok((file, size))
    }

    fn object_path(&self, oid: &str) -> Result<PathBuf, StoreError> {
        // Validate before using any part of the untrusted URL as a path. The
        // two prefix directories keep large stores from filling one directory.
        if oid.len() != 64
            || !oid
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(StoreError::InvalidOid);
        }
        Ok(self
            .root
            .join("objects")
            .join(&oid[..2])
            .join(&oid[2..4])
            .join(oid))
    }
}

// A failed upload removes its temporary file. Renaming within the destination
// directory exposes a complete object atomically to readers.
struct TempObject {
    path: PathBuf,
    file: Option<File>,
    committed: bool,
}

impl TempObject {
    fn create(directory: &Path) -> Result<Self, StoreError> {
        for _ in 0..10 {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = directory.join(format!(".upload-{}-{}", std::process::id(), id));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                        committed: false,
                    })
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(StoreError::Io(error)),
            }
        }
        Err(StoreError::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not create a unique upload file",
        )))
    }

    fn file_mut(&mut self) -> &mut File {
        self.file.as_mut().expect("temporary upload file is open")
    }

    fn commit(mut self, destination: &Path) -> Result<(), StoreError> {
        // Finish writing before the rename. Dropping the handle also allows
        // cleanup on platforms that cannot remove an open file.
        self.file
            .take()
            .expect("temporary upload file is open")
            .sync_all()
            .map_err(StoreError::Io)?;
        fs::rename(&self.path, destination).map_err(StoreError::Io)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for TempObject {
    fn drop(&mut self) {
        self.file.take();
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "local-lfs-store-test-{}-{}",
                std::process::id(),
                id
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn oid_for(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    #[test]
    fn stores_and_reopens_large_binary_object() {
        let directory = TestDirectory::new();
        let store = ObjectStore::new(&directory.0).unwrap();
        let bytes = vec![0xff; 128 * 1024 + 3];
        let oid = oid_for(&bytes);
        store
            .put(&oid, bytes.len() as u64, &mut Cursor::new(&bytes))
            .unwrap();

        // A fresh store instance must read what the first one persisted.
        let reopened = ObjectStore::new(&directory.0).unwrap();
        let (mut file, size) = reopened.open(&oid).unwrap();
        let mut actual = Vec::new();
        file.read_to_end(&mut actual).unwrap();
        assert_eq!(size, bytes.len() as u64);
        assert_eq!(actual, bytes);
    }

    #[test]
    fn rejects_wrong_size_and_hash_without_publishing() {
        let directory = TestDirectory::new();
        let store = ObjectStore::new(&directory.0).unwrap();
        let bytes = b"hello";
        let oid = oid_for(bytes);
        let result = store.put(&oid, 6, &mut Cursor::new(bytes));
        assert!(matches!(
            result,
            Err(StoreError::SizeMismatch {
                expected: 6,
                actual: 5
            })
        ));
        assert!(matches!(store.open(&oid), Err(StoreError::NotFound)));

        let wrong_oid = oid_for(b"other");
        let result = store.put(&wrong_oid, bytes.len() as u64, &mut Cursor::new(bytes));
        assert!(matches!(result, Err(StoreError::HashMismatch)));
        assert!(matches!(store.open(&wrong_oid), Err(StoreError::NotFound)));

        // Both failures must also remove the unpublished temporary files.
        for id in [&oid, &wrong_oid] {
            let shard = store.object_path(id).unwrap();
            let names: Vec<_> = fs::read_dir(shard.parent().unwrap())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            assert!(names.is_empty());
        }
    }

    #[test]
    fn rejects_path_like_and_noncanonical_ids() {
        let directory = TestDirectory::new();
        let store = ObjectStore::new(&directory.0).unwrap();
        for oid in ["../outside", "not-a-hash", &"A".repeat(64)] {
            assert!(matches!(store.open(oid), Err(StoreError::InvalidOid)));
            assert!(matches!(
                store.put(oid, 0, &mut Cursor::new(b"")),
                Err(StoreError::InvalidOid)
            ));
        }
    }
}
