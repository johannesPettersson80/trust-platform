use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf, ptr};

use windows_sys::{
    core::GUID,
    Win32::{
        System::{Com::CoTaskMemFree, SystemInformation::GetSystemDirectoryW},
        UI::Shell::{
            FOLDERID_ProgramData, FOLDERID_ProgramFiles, FOLDERID_ProgramFilesX86,
            SHGetKnownFolderPath,
        },
    },
};

use crate::{
    install_paths::{
        trusted_program_data_with_provider, WindowsDirectory, WindowsDirectoryProvider,
    },
    AdsError,
};

const MAX_WINDOWS_PATH_UNITS: usize = 32_768;

pub(crate) struct NativeWindowsDirectoryProvider;

impl WindowsDirectoryProvider for NativeWindowsDirectoryProvider {
    fn directory(&self, directory: WindowsDirectory) -> Result<PathBuf, String> {
        match directory {
            WindowsDirectory::System32 => system_directory(),
            WindowsDirectory::ProgramFiles => known_folder("Program Files", &FOLDERID_ProgramFiles),
            WindowsDirectory::ProgramFilesX86 => {
                known_folder("Program Files (x86)", &FOLDERID_ProgramFilesX86)
            }
            WindowsDirectory::ProgramData => known_folder("ProgramData", &FOLDERID_ProgramData),
        }
    }
}

pub(crate) fn trusted_program_data_root() -> Result<PathBuf, AdsError> {
    trusted_program_data_with_provider(&NativeWindowsDirectoryProvider).map_err(|reason| {
        AdsError::WindowsDirectory {
            directory: "ProgramData",
            reason,
        }
    })
}

fn known_folder(name: &'static str, folder_id: &GUID) -> Result<PathBuf, String> {
    let mut raw: *mut u16 = ptr::null_mut();
    // SAFETY: `folder_id` is a live Windows KNOWNFOLDERID and `raw` is an
    // exclusive PWSTR output. A null process token requests the current user's
    // OS-defined view; no process environment variables participate.
    let result = unsafe { SHGetKnownFolderPath(folder_id, 0, ptr::null_mut(), &raw mut raw) };
    if result < 0 {
        // SAFETY: SHGetKnownFolderPath permits freeing its output on failure;
        // CoTaskMemFree is a no-op for null.
        unsafe { CoTaskMemFree(raw.cast()) };
        return Err(format!(
            "SHGetKnownFolderPath({name}) failed with HRESULT 0x{:08X}",
            result.cast_unsigned()
        ));
    }
    if raw.is_null() {
        return Err(format!("SHGetKnownFolderPath({name}) returned null"));
    }

    // SAFETY: a successful SHGetKnownFolderPath returns a NUL-terminated PWSTR.
    // The explicit Windows maximum prevents unbounded scanning if the OS
    // contract is violated; the pointer remains owned until CoTaskMemFree below.
    let decoded = unsafe {
        let mut length = 0;
        while length < MAX_WINDOWS_PATH_UNITS && *raw.add(length) != 0 {
            length += 1;
        }
        if length == MAX_WINDOWS_PATH_UNITS {
            Err(format!(
                "SHGetKnownFolderPath({name}) returned an oversized path"
            ))
        } else {
            let units = std::slice::from_raw_parts(raw, length);
            Ok(PathBuf::from(OsString::from_wide(units)))
        }
    };
    // SAFETY: `raw` is the exact allocation returned by SHGetKnownFolderPath
    // and has not previously been freed.
    unsafe { CoTaskMemFree(raw.cast()) };
    decoded
}

fn system_directory() -> Result<PathBuf, String> {
    let mut buffer = vec![0_u16; 260];
    loop {
        let capacity = u32::try_from(buffer.len())
            .map_err(|_| "Windows system-directory buffer exceeds u32".to_string())?;
        // SAFETY: the buffer is live and writable for exactly `capacity` UTF-16
        // units and GetSystemDirectoryW does not retain the pointer.
        let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), capacity) };
        if length == 0 {
            return Err(format!(
                "GetSystemDirectoryW failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        let length = usize::try_from(length)
            .map_err(|_| "Windows returned an invalid system-directory length".to_string())?;
        if length < buffer.len() {
            return Ok(PathBuf::from(OsString::from_wide(&buffer[..length])));
        }
        if length >= MAX_WINDOWS_PATH_UNITS {
            return Err("GetSystemDirectoryW returned an oversized path".to_string());
        }
        buffer.resize(length.saturating_add(1), 0);
    }
}
