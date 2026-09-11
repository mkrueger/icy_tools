use std::{io::Write, path::Path};

pub fn same_file(first: &Path, second: &Path) -> bool {
    first == second || first.canonicalize().ok().is_some_and(|first| second.canonicalize().ok() == Some(first))
}

pub fn save_bytes(path: &Path, bytes: &[u8], original: Option<(&Path, &[u8])>, overwrite: bool) -> Result<(), String> {
    let target = match path.canonicalize() {
        Ok(target) => target,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => path.to_path_buf(),
        Err(error) => return Err(error.to_string()),
    };
    let previous = match std::fs::read(&target) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
    if !overwrite {
        match original.filter(|(source, _)| same_file(source, path)) {
            Some((_, baseline)) if previous.as_deref() != Some(baseline) => {
                return Err("The file was changed or removed outside Icy Draw. Use Save As to confirm replacement.".into())
            }
            None if previous.is_some() => return Err("The file already exists. Choose another name or confirm replacement.".into()),
            _ => {}
        }
    }
    let parent = target.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary.write_all(bytes).map_err(|error| error.to_string())?;
    if let Ok(metadata) = std::fs::metadata(&target) {
        temporary.as_file().set_permissions(metadata.permissions()).map_err(|error| error.to_string())?;
    }
    temporary.as_file().sync_all().map_err(|error| error.to_string())?;
    if previous.is_none() && !overwrite {
        temporary.persist_noclobber(&target).map_err(|error| error.to_string())?;
    } else {
        temporary.persist(&target).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_external_changes_and_detects_removal() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("art.icy");
        save_bytes(&path, b"initial", None, false).unwrap();
        assert!(save_bytes(&path, b"other", None, false).is_err());
        save_bytes(&path, b"saved", Some((&path, b"initial")), false).unwrap();
        assert!(save_bytes(&path, b"lost", Some((&path, b"initial")), false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"saved");
        std::fs::remove_file(&path).unwrap();
        assert!(save_bytes(&path, b"lost", Some((&path, b"saved")), false).is_err());
        assert!(!path.exists());
        save_bytes(&path, b"confirmed", None, true).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn follows_symlinks_and_preserves_permissions() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("art.icy");
        let link = directory.path().join("alias.icy");
        std::fs::write(&path, b"initial").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        symlink(&path, &link).unwrap();
        save_bytes(&link, b"saved", Some((&path, b"initial")), false).unwrap();
        assert!(link.is_symlink());
        assert_eq!(std::fs::read(&path).unwrap(), b"saved");
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o640);
    }
}
