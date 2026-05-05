impl RetainSnapshot {
    pub fn from_runtime(runtime: &Runtime) -> Self {
        runtime.retain_snapshot()
    }
}

static RETAIN_TMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// File-based retain store.
#[derive(Debug, Clone)]
pub struct FileRetainStore {
    path: PathBuf,
}

impl FileRetainStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), RuntimeError> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|err| {
            RuntimeError::RetainStore(format!("create retain dir {parent:?}: {err}").into())
        })?;
        let tmp_path = temp_retain_path(path);
        let write_result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(|err| {
                    RuntimeError::RetainStore(format!("create temp retain {tmp_path:?}: {err}").into())
                })?;
            file.write_all(bytes).map_err(|err| {
                RuntimeError::RetainStore(format!("write temp retain {tmp_path:?}: {err}").into())
            })?;
            file.flush().map_err(|err| {
                RuntimeError::RetainStore(format!("flush temp retain {tmp_path:?}: {err}").into())
            })?;
            file.sync_all().map_err(|err| {
                RuntimeError::RetainStore(format!("fsync temp retain {tmp_path:?}: {err}").into())
            })?;
            drop(file);
            fs::rename(&tmp_path, path).map_err(|err| {
                RuntimeError::RetainStore(
                    format!("atomic rename retain {tmp_path:?} to {path:?}: {err}").into(),
                )
            })?;
            sync_parent_dir(parent)?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&tmp_path);
        }
        write_result
    }

    fn read_bytes(path: &Path) -> Result<Vec<u8>, RuntimeError> {
        let mut file = fs::File::open(path)
            .map_err(|err| RuntimeError::RetainStore(format!("open {path:?}: {err}").into()))?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|err| RuntimeError::RetainStore(format!("read {path:?}: {err}").into()))?;
        Ok(buf)
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

fn sync_parent_dir(path: &Path) -> Result<(), RuntimeError> {
    #[cfg(unix)]
    {
        let dir = fs::File::open(path).map_err(|err| {
            RuntimeError::RetainStore(format!("open retain dir {path:?}: {err}").into())
        })?;
        dir.sync_all().map_err(|err| {
            RuntimeError::RetainStore(format!("fsync retain dir {path:?}: {err}").into())
        })?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

impl RetainStore for FileRetainStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        if !self.path.exists() {
            return Ok(RetainSnapshot::default());
        }
        let bytes = Self::read_bytes(&self.path)?;
        decode_snapshot(&bytes)
    }

    fn store(&self, snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        let bytes = encode_snapshot(snapshot)?;
        Self::write_bytes(&self.path, &bytes)
    }
}
