fn collect_file_digests(root: &Path) -> anyhow::Result<Vec<PackageFileDigest>> {
    let mut files = Vec::new();
    collect_file_digests_inner(root, root, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_file_digests_inner(
    root: &Path,
    current: &Path,
    out: &mut Vec<PackageFileDigest>,
) -> anyhow::Result<()> {
    for entry in
        fs::read_dir(current).with_context(|| format!("failed to read {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_file_digests_inner(root, &path, out)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .with_context(|| format!("failed to relativize {}", path.display()))?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = path
            .metadata()
            .with_context(|| format!("failed to stat {}", path.display()))?
            .len();
        let sha256 = sha256_file(&path)?;
        out.push(PackageFileDigest {
            path: relative,
            bytes,
            sha256,
        });
    }
    Ok(())
}

fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_string(&hasher.finalize()))
}

fn aggregate_package_sha(files: &[PackageFileDigest]) -> String {
    let mut hasher = Sha256::new();
    for file in files {
        hasher.update(file.path.as_bytes());
        hasher.update([0_u8]);
        hasher.update(file.sha256.as_bytes());
        hasher.update([0_u8]);
        hasher.update(file.bytes.to_le_bytes());
    }
    hex_string(&hasher.finalize())
}

fn verify_bundle_tree_against_metadata(
    bundle_root: &Path,
    metadata: &PackageMetadata,
) -> anyhow::Result<()> {
    let digests = collect_file_digests(bundle_root)?;
    if digests.len() != metadata.files.len() {
        anyhow::bail!(
            "package verification failed: expected {} files, found {}",
            metadata.files.len(),
            digests.len()
        );
    }
    for (expected, actual) in metadata.files.iter().zip(digests.iter()) {
        if expected.path != actual.path {
            anyhow::bail!(
                "package verification failed: path mismatch '{}' != '{}'",
                expected.path,
                actual.path
            );
        }
        if expected.bytes != actual.bytes {
            anyhow::bail!(
                "package verification failed: '{}' size mismatch {} != {}",
                expected.path,
                expected.bytes,
                actual.bytes
            );
        }
        if expected.sha256 != actual.sha256 {
            anyhow::bail!(
                "package verification failed: '{}' digest mismatch",
                expected.path
            );
        }
    }
    let actual_package_sha = aggregate_package_sha(&digests);
    if metadata.package_sha256 != actual_package_sha {
        anyhow::bail!("package verification failed: package_sha256 mismatch");
    }
    Ok(())
}
