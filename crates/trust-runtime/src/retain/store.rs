impl RetainSnapshot {
    pub fn from_runtime(runtime: &Runtime) -> Self {
        runtime.retain_snapshot()
    }
}

static RETAIN_TMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

trait RetainTempFile: Write + Send {
    fn sync_all(&self) -> std::io::Result<()>;
}

impl RetainTempFile for fs::File {
    fn sync_all(&self) -> std::io::Result<()> {
        fs::File::sync_all(self)
    }
}

trait RetainFileOps: Send + Sync {
    fn create_dir_all(&self, path: &Path) -> std::io::Result<()>;
    fn create_temp(&self, path: &Path) -> std::io::Result<Box<dyn RetainTempFile>>;
    fn open_read(&self, path: &Path) -> std::io::Result<Box<dyn Read + Send>>;
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()>;
    fn remove_file(&self, path: &Path) -> std::io::Result<()>;
    fn sync_parent(&self, path: &Path) -> std::io::Result<()>;
}

#[derive(Debug)]
struct StdRetainFileOps;

impl RetainFileOps for StdRetainFileOps {
    fn create_dir_all(&self, path: &Path) -> std::io::Result<()> {
        fs::create_dir_all(path)
    }

    fn create_temp(&self, path: &Path) -> std::io::Result<Box<dyn RetainTempFile>> {
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .map(|file| Box::new(file) as Box<dyn RetainTempFile>)
    }

    fn open_read(&self, path: &Path) -> std::io::Result<Box<dyn Read + Send>> {
        fs::File::open(path).map(|file| Box::new(file) as Box<dyn Read + Send>)
    }

    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        fs::rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> std::io::Result<()> {
        fs::remove_file(path)
    }

    fn sync_parent(&self, path: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            fs::File::open(path)?.sync_all()
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Ok(())
        }
    }
}

/// File-based retain store.
#[derive(Clone)]
pub struct FileRetainStore {
    path: PathBuf,
    file_ops: std::sync::Arc<dyn RetainFileOps>,
}

impl std::fmt::Debug for FileRetainStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileRetainStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl FileRetainStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            file_ops: std::sync::Arc::new(StdRetainFileOps),
        }
    }

    #[cfg(test)]
    fn with_file_ops(path: impl Into<PathBuf>, file_ops: std::sync::Arc<dyn RetainFileOps>) -> Self {
        Self {
            path: path.into(),
            file_ops,
        }
    }

    fn write_bytes(&self, path: &Path, bytes: &[u8]) -> Result<(), RuntimeError> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        self.file_ops.create_dir_all(parent).map_err(|err| {
            RuntimeError::RetainStore(format!("create retain dir {parent:?}: {err}").into())
        })?;
        let tmp_path = temp_retain_path(path);
        let write_result = (|| {
            let mut file = self.file_ops.create_temp(&tmp_path).map_err(|err| {
                    RuntimeError::RetainStore(format!("create temp retain {tmp_path:?}: {err}").into())
                })?;
            file.write_all(bytes).map_err(|err| {
                RuntimeError::RetainStore(format!("write temp retain {tmp_path:?}: {err}").into())
            })?;
            file.flush().map_err(|err| {
                RuntimeError::RetainStore(format!("flush temp retain {tmp_path:?}: {err}").into())
            })?;
            RetainTempFile::sync_all(file.as_ref()).map_err(|err| {
                RuntimeError::RetainStore(format!("fsync temp retain {tmp_path:?}: {err}").into())
            })?;
            drop(file);
            self.file_ops.rename(&tmp_path, path).map_err(|err| {
                RuntimeError::RetainStore(
                    format!("atomic rename retain {tmp_path:?} to {path:?}: {err}").into(),
                )
            })?;
            self.file_ops.sync_parent(parent).map_err(|err| {
                RuntimeError::RetainStore(format!("fsync retain dir {parent:?}: {err}").into())
            })?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = self.file_ops.remove_file(&tmp_path);
        }
        write_result
    }

    fn read_bytes(&self, path: &Path) -> Result<Option<Vec<u8>>, RuntimeError> {
        let mut file = match self.file_ops.open_read(path) {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(RuntimeError::RetainStore(
                    format!("open {path:?}: {err}").into(),
                ));
            }
        };
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|err| RuntimeError::RetainStore(format!("read {path:?}: {err}").into()))?;
        Ok(Some(buf))
    }
}

fn temp_retain_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("retain");
    let seq = RETAIN_TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    parent.join(format!(".{file_name}.{}.{seq}.tmp", std::process::id()))
}

impl RetainStore for FileRetainStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        let Some(bytes) = self.read_bytes(&self.path)? else {
            return Ok(RetainSnapshot::default());
        };
        decode_snapshot(&bytes)
    }

    fn store(&self, snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        let bytes = encode_snapshot(snapshot)?;
        self.write_bytes(&self.path, &bytes)
    }
}
