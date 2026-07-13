use std::{collections::HashSet, path::PathBuf};

const DLL_NAME: &str = "TcAdsDll.dll";
const FIXED_TWINCAT_ROOTS: [&str; 3] = [
    r"C:\TwinCAT",
    r"C:\Program Files\Beckhoff\TwinCAT",
    r"C:\Program Files (x86)\Beckhoff\TwinCAT",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WindowsDirectory {
    System32,
    ProgramFiles,
    ProgramFilesX86,
    ProgramData,
}

pub(crate) trait WindowsDirectoryProvider {
    fn directory(&self, directory: WindowsDirectory) -> Result<PathBuf, String>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DllLoadStrategy {
    System32Search,
    AbsoluteTrustedPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TrustedDllCandidate {
    path: PathBuf,
    strategy: DllLoadStrategy,
}

impl TrustedDllCandidate {
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub(crate) const fn strategy(&self) -> DllLoadStrategy {
        self.strategy
    }
}

pub(crate) struct TrustedPathPolicy {
    candidates: Vec<TrustedDllCandidate>,
}

impl TrustedPathPolicy {
    pub(crate) fn from_provider(provider: &impl WindowsDirectoryProvider) -> Self {
        let mut candidates = Vec::new();
        match absolute_directory(provider, WindowsDirectory::System32) {
            Some(system32) => candidates.push(TrustedDllCandidate {
                path: system32.join(DLL_NAME),
                strategy: DllLoadStrategy::AbsoluteTrustedPath,
            }),
            None => candidates.push(TrustedDllCandidate {
                path: PathBuf::from(DLL_NAME),
                strategy: DllLoadStrategy::System32Search,
            }),
        }

        let mut roots = Vec::new();
        for directory in [
            WindowsDirectory::ProgramFiles,
            WindowsDirectory::ProgramFilesX86,
        ] {
            if let Some(program_files) = absolute_directory(provider, directory) {
                roots.push(program_files.join("Beckhoff").join("TwinCAT"));
            }
        }
        roots.extend(FIXED_TWINCAT_ROOTS.map(PathBuf::from));

        let mut seen = HashSet::new();
        seen.insert(normalized_windows_path(&candidates[0].path));
        for root in roots {
            for path in dll_candidates_under(&root) {
                if seen.insert(normalized_windows_path(&path)) {
                    candidates.push(TrustedDllCandidate {
                        path,
                        strategy: DllLoadStrategy::AbsoluteTrustedPath,
                    });
                }
            }
        }
        Self { candidates }
    }

    pub(crate) fn candidates(&self) -> &[TrustedDllCandidate] {
        &self.candidates
    }

    #[cfg(any(windows, test))]
    pub(crate) fn searched_paths(&self) -> Vec<PathBuf> {
        self.candidates
            .iter()
            .map(|candidate| candidate.path.clone())
            .collect()
    }

    #[cfg(test)]
    fn contains_path(&self, path: &std::path::Path) -> bool {
        let path = normalized_windows_path(path);
        self.candidates
            .iter()
            .any(|candidate| normalized_windows_path(&candidate.path) == path)
    }
}

pub(crate) fn trusted_program_data_with_provider(
    provider: &impl WindowsDirectoryProvider,
) -> Result<PathBuf, String> {
    let path = provider.directory(WindowsDirectory::ProgramData)?;
    if is_absolute_windows_path(&path) {
        Ok(path)
    } else {
        Err("Windows returned a non-absolute ProgramData path".to_string())
    }
}

fn absolute_directory(
    provider: &impl WindowsDirectoryProvider,
    directory: WindowsDirectory,
) -> Option<PathBuf> {
    provider
        .directory(directory)
        .ok()
        .filter(|path| is_absolute_windows_path(path))
}

fn dll_candidates_under(root: &std::path::Path) -> [PathBuf; 4] {
    let architecture = if cfg!(target_pointer_width = "64") {
        "x64"
    } else {
        "x86"
    };
    let common = if cfg!(target_pointer_width = "64") {
        "Common64"
    } else {
        "Common32"
    };
    [
        root.join("AdsApi")
            .join("TcAdsDll")
            .join(architecture)
            .join(DLL_NAME),
        root.join("AdsApi").join("TcAdsDll").join(DLL_NAME),
        root.join("3.1").join("System").join(DLL_NAME),
        root.join(common).join(DLL_NAME),
    ]
}

fn is_absolute_windows_path(path: &std::path::Path) -> bool {
    let text = path.as_os_str().to_string_lossy();
    let bytes = text.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
}

fn normalized_windows_path(path: &std::path::Path) -> String {
    path.as_os_str()
        .to_string_lossy()
        .replace('/', r"\")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn os_known_folders_support_non_c_windows_roots_and_reject_arbitrary_paths() {
        let policy = TrustedPathPolicy::from_provider(&FakeDirectoryProvider);
        let architecture = if cfg!(target_pointer_width = "64") {
            "x64"
        } else {
            "x86"
        };

        assert!(policy.contains_path(std::path::Path::new(r"D:\Windows\System32\TcAdsDll.dll")));
        assert!(policy.contains_path(std::path::Path::new(&format!(
            r"D:\Program Files\Beckhoff\TwinCAT\AdsApi\TcAdsDll\{architecture}\TcAdsDll.dll"
        ))));
        assert!(policy.contains_path(std::path::Path::new(
            r"D:\Program Files (x86)\Beckhoff\TwinCAT\3.1\System\TcAdsDll.dll"
        )));
        assert!(!policy.contains_path(std::path::Path::new(r"C:\Users\attacker\TcAdsDll.dll")));
        assert!(!policy.contains_path(std::path::Path::new("TcAdsDll.dll")));
        assert_eq!(
            trusted_program_data_with_provider(&FakeDirectoryProvider),
            Ok(PathBuf::from(r"D:\ProgramData"))
        );
    }

    struct MissingDirectoryProvider;

    impl WindowsDirectoryProvider for MissingDirectoryProvider {
        fn directory(&self, _directory: WindowsDirectory) -> Result<PathBuf, String> {
            Err("unavailable".to_string())
        }
    }

    #[test]
    fn fixed_roots_and_system32_only_search_are_safe_fallbacks() {
        let policy = TrustedPathPolicy::from_provider(&MissingDirectoryProvider);
        let common = if cfg!(target_pointer_width = "64") {
            "Common64"
        } else {
            "Common32"
        };

        assert_eq!(
            policy.candidates()[0].strategy(),
            DllLoadStrategy::System32Search
        );
        assert_eq!(
            policy.candidates()[0].path(),
            std::path::Path::new(DLL_NAME)
        );
        assert!(policy.contains_path(std::path::Path::new(r"C:\TwinCAT\3.1\System\TcAdsDll.dll")));
        assert!(policy.contains_path(std::path::Path::new(&format!(
            r"C:\Program Files\Beckhoff\TwinCAT\{common}\TcAdsDll.dll"
        ))));
    }

    #[test]
    fn program_data_must_be_an_absolute_os_path() {
        struct RelativeProgramData;
        impl WindowsDirectoryProvider for RelativeProgramData {
            fn directory(&self, _directory: WindowsDirectory) -> Result<PathBuf, String> {
                Ok(PathBuf::from(r"Users\attacker\ProgramData"))
            }
        }

        assert_eq!(
            trusted_program_data_with_provider(&RelativeProgramData),
            Err("Windows returned a non-absolute ProgramData path".to_string())
        );
    }
}
