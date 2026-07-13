use std::path::PathBuf;

use super::*;
use crate::install_paths::{WindowsDirectory, WindowsDirectoryProvider};

struct FakeDirectoryProvider;

impl WindowsDirectoryProvider for FakeDirectoryProvider {
    fn directory(&self, directory: WindowsDirectory) -> Result<PathBuf, String> {
        Ok(PathBuf::from(match directory {
            WindowsDirectory::System32 => r"D:\Windows\System32",
            WindowsDirectory::ProgramFiles => r"D:\Program Files",
            WindowsDirectory::ProgramFilesX86 => r"D:\Program Files (x86)",
            WindowsDirectory::ProgramData => r"D:\ProgramData",
        }))
    }
}

fn policy() -> TrustedPathPolicy {
    TrustedPathPolicy::from_provider(&FakeDirectoryProvider)
}

#[test]
fn stale_system32_candidate_falls_through_to_compatible_beckhoff_candidate() {
    let policy = policy();
    let system32 = policy.candidates()[0].path().to_path_buf();
    let beckhoff = policy
        .candidates()
        .iter()
        .find(|candidate| {
            candidate
                .path()
                .to_string_lossy()
                .replace('/', r"\")
                .starts_with(r"D:\Program Files\Beckhoff\TwinCAT")
        })
        .expect("trusted Beckhoff candidate")
        .path()
        .to_path_buf();
    let mut loaded = Vec::new();

    let selected = load_first_compatible_candidate(
        &policy,
        |_| true,
        |candidate| {
            let path = candidate.path().to_path_buf();
            loaded.push(path.clone());
            Ok(path)
        },
        |library_path| {
            if library_path == system32 {
                Err(AdsError::MissingSymbol {
                    symbol: "AdsSyncReadWriteReqEx2",
                    reason: "stale System32 export table".to_string(),
                })
            } else if library_path == beckhoff {
                Ok(library_path)
            } else {
                Err(AdsError::MissingSymbol {
                    symbol: "AdsSyncReadWriteReqEx2",
                    reason: "incompatible trusted candidate".to_string(),
                })
            }
        },
    )
    .expect("compatible Beckhoff candidate must be selected");

    assert_eq!(selected, beckhoff);
    assert_eq!(loaded, vec![system32, beckhoff]);
}

#[test]
fn symbol_resolution_error_is_preferred_when_no_candidate_is_compatible() {
    let policy = policy();
    let system32 = policy.candidates()[0].path().to_path_buf();

    let error = load_first_compatible_candidate(
        &policy,
        |_| true,
        |candidate| {
            if candidate.path() == system32 {
                Ok(candidate.path().to_path_buf())
            } else {
                Err("candidate could not be loaded".to_string())
            }
        },
        |_| {
            Err::<PathBuf, _>(AdsError::MissingSymbol {
                symbol: "AdsSyncReadWriteReqEx2",
                reason: "stale export table".to_string(),
            })
        },
    )
    .expect_err("no compatible candidate");

    assert!(matches!(
        error,
        AdsError::MissingSymbol {
            symbol: "AdsSyncReadWriteReqEx2",
            ..
        }
    ));
}
