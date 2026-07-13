#[cfg(windows)]
use std::sync::Arc;

#[cfg(windows)]
use libloading::os::windows::{
    Library, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
};

#[cfg(windows)]
use crate::{
    ffi::{FunctionOwner, FunctionTable},
    windows_paths::NativeWindowsDirectoryProvider,
};
use crate::{
    install_paths::{DllLoadStrategy, TrustedDllCandidate, TrustedPathPolicy},
    AdsError,
};

#[cfg(windows)]
pub(crate) fn load_installed() -> Result<Arc<dyn FunctionOwner>, AdsError> {
    let policy = TrustedPathPolicy::from_provider(&NativeWindowsDirectoryProvider);
    load_first_compatible_candidate(
        &policy,
        |candidate| candidate.path().is_file(),
        load_candidate,
        owner,
    )
}

fn load_first_compatible_candidate<LibraryHandle, Owner>(
    policy: &TrustedPathPolicy,
    mut candidate_exists: impl FnMut(&TrustedDllCandidate) -> bool,
    mut load: impl FnMut(&TrustedDllCandidate) -> Result<LibraryHandle, String>,
    mut resolve: impl FnMut(LibraryHandle) -> Result<Owner, AdsError>,
) -> Result<Owner, AdsError> {
    let mut last_load_error = None;
    let mut last_resolution_error = None;
    for candidate in policy.candidates() {
        if candidate.strategy() == DllLoadStrategy::AbsoluteTrustedPath
            && !candidate_exists(candidate)
        {
            continue;
        }
        match load(candidate) {
            Ok(library) => match resolve(library) {
                Ok(owner) => return Ok(owner),
                Err(error) => last_resolution_error = Some(error),
            },
            Err(reason) => {
                last_load_error = Some((candidate.path().to_path_buf(), reason));
            }
        }
    }

    if let Some(error) = last_resolution_error {
        Err(error)
    } else if let Some((path, reason)) = last_load_error {
        Err(AdsError::LibraryLoad { path, reason })
    } else {
        Err(AdsError::LibraryNotFound {
            searched: policy.searched_paths(),
        })
    }
}

#[cfg(windows)]
struct NativeFunctionOwner {
    functions: FunctionTable,
    _library: Library,
}

#[cfg(windows)]
impl FunctionOwner for NativeFunctionOwner {
    fn functions(&self) -> &FunctionTable {
        &self.functions
    }
}

#[cfg(windows)]
fn owner(library: Library) -> Result<Arc<dyn FunctionOwner>, AdsError> {
    let functions = FunctionTable {
        port_open: symbol(&library, b"AdsPortOpenEx\0", "AdsPortOpenEx")?,
        port_close: symbol(&library, b"AdsPortCloseEx\0", "AdsPortCloseEx")?,
        get_local_address: symbol(&library, b"AdsGetLocalAddressEx\0", "AdsGetLocalAddressEx")?,
        set_timeout: symbol(&library, b"AdsSyncSetTimeoutEx\0", "AdsSyncSetTimeoutEx")?,
        read_state: symbol(
            &library,
            b"AdsSyncReadStateReqEx\0",
            "AdsSyncReadStateReqEx",
        )?,
        read: symbol(&library, b"AdsSyncReadReqEx2\0", "AdsSyncReadReqEx2")?,
        write: symbol(&library, b"AdsSyncWriteReqEx\0", "AdsSyncWriteReqEx")?,
        read_write: symbol(
            &library,
            b"AdsSyncReadWriteReqEx2\0",
            "AdsSyncReadWriteReqEx2",
        )?,
    };
    Ok(Arc::new(NativeFunctionOwner {
        functions,
        _library: library,
    }))
}

#[cfg(windows)]
fn load_candidate(candidate: &TrustedDllCandidate) -> Result<Library, String> {
    let flags = match candidate.strategy() {
        DllLoadStrategy::System32Search => LOAD_LIBRARY_SEARCH_SYSTEM32,
        DllLoadStrategy::AbsoluteTrustedPath => {
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32
        }
    };
    // SAFETY: TrustedDllCandidate has no public constructor. Its path is either
    // the fixed DLL name paired with SYSTEM32-only search, or an absolute path
    // derived from trusted Windows APIs/fixed fallback roots. Dependencies for
    // absolute candidates search only beside the DLL and in SYSTEM32.
    unsafe { Library::load_with_flags(candidate.path().as_os_str(), flags) }
        .map_err(|error| error.to_string())
}

#[cfg(windows)]
fn symbol<T: Copy>(
    library: &Library,
    bytes: &'static [u8],
    name: &'static str,
) -> Result<T, AdsError> {
    // SAFETY: each call site supplies the exact Beckhoff extern "system"
    // signature for `name`; FunctionTable cannot outlive `library` because both
    // are owned by NativeFunctionOwner.
    unsafe { library.get::<T>(bytes) }
        .map(|symbol| *symbol)
        .map_err(|error| AdsError::MissingSymbol {
            symbol: name,
            reason: error.to_string(),
        })
}

#[cfg(test)]
#[path = "native/tests.rs"]
mod tests;
