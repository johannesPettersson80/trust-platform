//! Reuse pristine, driverless HMI compilations between reads of unchanged source text.
//! A mutable borrow irrevocably excludes that runtime from reuse.
use super::LoadedSource;
use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use trust_runtime::Runtime;

const CAPACITY: usize = 2;
const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq)]
struct Key {
    root: PathBuf,
    sources: Vec<(PathBuf, String)>,
}
struct Entry {
    key: Key,
    runtime: Runtime,
}
#[derive(Default)]
struct CacheState {
    entries: VecDeque<Entry>,
    active: Vec<Key>,
}
#[derive(Default)]
struct Cache {
    state: Mutex<CacheState>,
    available: Condvar,
}
struct Lease {
    cache: Arc<Cache>,
    key: Key,
}
impl Drop for Lease {
    fn drop(&mut self) {
        let mut state = self
            .cache
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.active.retain(|key| key != &self.key);
        self.cache.available.notify_all();
    }
}

pub(super) struct CachedRuntime {
    runtime: Option<Runtime>,
    owner: Option<Lease>,
}
impl From<Runtime> for CachedRuntime {
    fn from(runtime: Runtime) -> Self {
        Self {
            runtime: Some(runtime),
            owner: None,
        }
    }
}
impl Deref for CachedRuntime {
    type Target = Runtime;
    fn deref(&self) -> &Runtime {
        self.runtime.as_ref().expect("owned compilation")
    }
}
impl DerefMut for CachedRuntime {
    fn deref_mut(&mut self) -> &mut Runtime {
        if self.owner.is_some() {
            trust_hir::timing::event("cache.discard_on_mutable_access", 1);
        }
        self.owner = None;
        self.runtime.as_mut().expect("owned compilation")
    }
}
impl Drop for CachedRuntime {
    fn drop(&mut self) {
        let Some(owner) = self.owner.take() else {
            return;
        };
        if let Some(runtime) = self.runtime.take() {
            let mut state = owner
                .cache
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            state.entries.retain(|entry| entry.key != owner.key);
            state.entries.push_front(Entry {
                key: owner.key.clone(),
                runtime,
            });
            state.entries.truncate(CAPACITY);
        }
        // Releasing the lease wakes exact-source waiters only after restoring the pristine runtime.
    }
}
impl Cache {
    fn compile(
        self: &Arc<Self>,
        root: &Path,
        sources: &[LoadedSource],
        build: impl FnOnce() -> Result<Runtime, String>,
    ) -> Result<CachedRuntime, String> {
        if sources
            .iter()
            .map(|source| source.text.len())
            .sum::<usize>()
            > MAX_SOURCE_BYTES
        {
            return build().map(Into::into);
        }
        let key = Key {
            root: root.to_owned(),
            sources: sources
                .iter()
                .map(|source| (source.path.clone(), source.text.clone()))
                .collect(),
        };
        let found = {
            let _wait = trust_hir::timing::CompilerStage::new("hmi.cache.acquire", sources.len());
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            while state.active.contains(&key) {
                trust_hir::timing::event("cache.wait", 1);
                state = self
                    .available
                    .wait(state)
                    .unwrap_or_else(|error| error.into_inner());
            }
            state.active.push(key.clone());
            state
                .entries
                .iter()
                .position(|entry| entry.key == key)
                .and_then(|index| state.entries.remove(index))
        };
        // Errors and unwinding release the lease too. Different source snapshots never share state.
        let owner = Lease {
            cache: self.clone(),
            key,
        };

        let runtime = match found {
            Some(entry) => {
                trust_hir::timing::event("cache.hit", 1);
                entry.runtime
            }
            None => {
                trust_hir::timing::event("cache.miss", 1);
                build()?
            }
        };
        Ok(CachedRuntime {
            runtime: Some(runtime),
            owner: Some(owner),
        })
    }
}

pub(super) fn compile(
    root: &Path,
    sources: &[LoadedSource],
    build: impl FnOnce() -> Result<Runtime, String>,
) -> Result<CachedRuntime, String> {
    static CACHE: OnceLock<Arc<Cache>> = OnceLock::new();
    CACHE
        .get_or_init(|| Arc::new(Cache::default()))
        .compile(root, sources, build)
}

/// Bindings include a bounded, driverless alarm-message inspection. Retain its immutable
/// response separately so it never consumes or contaminates a pristine runtime entry.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct BindingsKey {
    source: Key,
    sources_root: PathBuf,
}
type BindingResponses = VecDeque<(BindingsKey, serde_json::Value)>;
fn binding_responses() -> &'static Mutex<BindingResponses> {
    static RESPONSES: OnceLock<Mutex<BindingResponses>> = OnceLock::new();
    RESPONSES.get_or_init(|| Mutex::new(VecDeque::new()))
}
pub(super) fn bindings_key(
    root: &Path,
    sources_root: &Path,
    sources: &[LoadedSource],
) -> Option<BindingsKey> {
    (sources
        .iter()
        .map(|source| source.text.len())
        .sum::<usize>()
        <= MAX_SOURCE_BYTES)
        .then(|| BindingsKey {
            source: Key {
                root: root.to_owned(),
                sources: sources
                    .iter()
                    .map(|source| (source.path.clone(), source.text.clone()))
                    .collect(),
            },
            sources_root: sources_root.to_owned(),
        })
}
pub(super) fn read_bindings(key: &BindingsKey) -> Option<serde_json::Value> {
    let mut responses = binding_responses()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let index = responses.iter().position(|(stored, _)| stored == key)?;
    let entry = responses.remove(index)?;
    let value = entry.1.clone();
    responses.push_front(entry);
    Some(value)
}
pub(super) fn store_bindings(key: BindingsKey, value: serde_json::Value) {
    if value.get("ok").and_then(serde_json::Value::as_bool) != Some(true)
        || value.to_string().len() > 8 * 1024 * 1024
    {
        return;
    }
    let mut responses = binding_responses()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    responses.retain(|(stored, _)| stored != &key);
    responses.push_front((key, value));
    responses.truncate(CAPACITY);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use trust_runtime::harness::CompileSession;
    use trust_runtime::hmi::member_paths::{resolve_member, MemberPath};
    use trust_runtime::value::Value;

    #[test]
    fn unchanged_reads_reuse_compilation_but_edits_and_other_projects_do_not() {
        let cache = Arc::new(Cache::default());
        let builds = Cell::new(0);
        let source = |value: &str| {
            vec![LoadedSource {
                path: "src/main.st".into(),
                text: format!("PROGRAM Main VAR Number : INT := {value}; END_VAR END_PROGRAM"),
            }]
        };
        let one = source("1");
        let two = source("2");
        let compile = |root: &str, sources: &[LoadedSource]| {
            cache.compile(Path::new(root), sources, || {
                builds.set(builds.get() + 1);
                CompileSession::from_source(&sources[0].text)
                    .build_runtime()
                    .map_err(|error| error.to_string())
            })
        };
        drop(compile("first", &one).unwrap());
        drop(compile("first", &one).unwrap());
        assert_eq!(builds.get(), 1);
        let changed = compile("first", &two).unwrap();
        assert_eq!(
            resolve_member(
                &MemberPath::parse("Main.Number").unwrap(),
                changed.storage()
            ),
            Some(&Value::Int(2))
        );
        drop(changed);
        drop(compile("second", &two).unwrap());
        assert_eq!(builds.get(), 3);
        let invalid = source("missing_name");
        assert!(
            compile("first", &invalid).is_err(),
            "an invalid edited buffer must not reuse old declarations"
        );
        assert_eq!(builds.get(), 4);
        drop(compile("first", &one).unwrap());
        assert_eq!(
            builds.get(),
            5,
            "the oldest of three source versions was evicted"
        );
    }

    #[test]
    fn concurrent_unchanged_reads_compile_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = Arc::new(Cache::default());
        let builds = AtomicUsize::new(0);
        let start = std::sync::Barrier::new(4);
        let sources = vec![LoadedSource {
            path: "src/main.st".into(),
            text: "PROGRAM Main VAR Number : INT := 3; END_VAR END_PROGRAM".into(),
        }];
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    start.wait();
                    let runtime = cache
                        .compile(Path::new("concurrent"), &sources, || {
                            builds.fetch_add(1, Ordering::SeqCst);
                            CompileSession::from_source(&sources[0].text)
                                .build_runtime()
                                .map_err(|e| e.to_string())
                        })
                        .unwrap();
                    assert_eq!(
                        resolve_member(
                            &MemberPath::parse("Main.Number").unwrap(),
                            runtime.storage()
                        ),
                        Some(&Value::Int(3))
                    );
                });
            }
        });
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn executed_runtime_cannot_contaminate_later_metadata_reads() {
        let cache = Arc::new(Cache::default());
        let sources = vec![LoadedSource {
            path: "src/main.st".into(),
            text: "PROGRAM Main VAR Number : INT; END_VAR Number := 7; END_PROGRAM".into(),
        }];
        let builds = Cell::new(0);
        let compile = || {
            cache.compile(Path::new("execution"), &sources, || {
                builds.set(builds.get() + 1);
                CompileSession::from_source(&sources[0].text)
                    .build_runtime()
                    .map_err(|error| error.to_string())
            })
        };
        let mut first = compile().unwrap();
        let program = first
            .metadata_snapshot()
            .programs()
            .values()
            .next()
            .unwrap()
            .clone();
        first.execute_program(&program).unwrap();
        drop(first);
        let second = compile().unwrap();
        assert_eq!(builds.get(), 2);
        assert_eq!(
            resolve_member(&MemberPath::parse("Main.Number").unwrap(), second.storage()),
            Some(&Value::Int(0))
        );
    }
}
