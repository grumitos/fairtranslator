use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::io::ErrorKind;
#[cfg(windows)]
use std::os::windows::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};

#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

/// Single-writer guard for campaign workspaces.
///
/// Windows uses an OS-backed exclusive file handle, so a terminated process
/// releases the lock even if the marker file remains on disk.
pub struct WorkspaceRunLock {
    path: PathBuf,
    file: Option<std::fs::File>,
}

impl WorkspaceRunLock {
    pub fn acquire(workspace: &Path) -> Result<Self, String> {
        let path = workspace.join(".fairtranslator-workspace.lock");
        let (mut file, should_write_pid) = open_lock_file(workspace, &path)?;
        if should_write_pid {
            writeln!(file, "pid={}", std::process::id()).map_err(|error| {
                format!(
                    "No se pudo registrar el lock del workspace '{}': {error}",
                    workspace.display()
                )
            })?;
            file.flush().map_err(|error| {
                format!(
                    "No se pudo confirmar el lock del workspace '{}': {error}",
                    workspace.display()
                )
            })?;
        }

        Ok(Self {
            path,
            file: Some(file),
        })
    }
}

#[cfg(windows)]
fn open_lock_file(workspace: &Path, path: &Path) -> Result<(std::fs::File, bool), String> {
    let mut create_options = OpenOptions::new();
    create_options
        .read(true)
        .write(true)
        .create_new(true)
        .share_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);

    match create_options.open(path) {
        Ok(file) => return Ok((file, true)),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(lock_open_error(workspace, error)),
    }

    let mut existing_options = OpenOptions::new();
    existing_options
        .read(true)
        .write(false)
        .share_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    let file = existing_options
        .open(path)
        .map_err(|error| lock_open_error(workspace, error))?;
    validate_existing_windows_lock(workspace, &file)?;
    Ok((file, false))
}

#[cfg(windows)]
fn validate_existing_windows_lock(workspace: &Path, file: &std::fs::File) -> Result<(), String> {
    let metadata = file.metadata().map_err(|error| {
        format!(
            "No se pudo validar el lock del workspace '{}': {error}",
            workspace.display()
        )
    })?;
    let file_type = metadata.file_type();
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || file_type.is_symlink_dir()
        || file_type.is_symlink_file()
        || !file_type.is_file()
    {
        return Err(format!(
            "El lock del workspace '{}' no es un archivo regular seguro",
            workspace.display()
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn lock_open_error(workspace: &Path, error: std::io::Error) -> String {
    format!(
        "El workspace '{}' ya está en uso por otro proceso o no se puede bloquear: {error}",
        workspace.display()
    )
}

#[cfg(not(windows))]
fn open_lock_file(workspace: &Path, path: &Path) -> Result<(std::fs::File, bool), String> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    options
        .open(path)
        .map(|file| (file, true))
        .map_err(|error| {
            format!(
                "El workspace '{}' ya está en uso por otro proceso o no se puede bloquear: {error}",
                workspace.display()
            )
        })
}

impl Drop for WorkspaceRunLock {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_second_writer_and_releases_on_drop() {
        let temp = std::env::temp_dir().join(format!(
            "fairtranslator-workspace-lock-test-{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        std::fs::create_dir_all(&temp).unwrap();

        let first = WorkspaceRunLock::acquire(&temp).unwrap();
        assert!(WorkspaceRunLock::acquire(&temp).is_err());
        drop(first);
        let second = WorkspaceRunLock::acquire(&temp).unwrap();
        drop(second);

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[cfg(windows)]
    #[test]
    fn does_not_clobber_target_through_preexisting_lock_hard_link() {
        let temp = std::env::temp_dir().join(format!(
            "fairtranslator-workspace-lock-hard-link-test-{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        let victim = temp.join("victim.txt");
        std::fs::write(&victim, "important data\n").unwrap();
        std::fs::hard_link(&victim, temp.join(".fairtranslator-workspace.lock")).unwrap();

        let lock = WorkspaceRunLock::acquire(&temp).unwrap();
        drop(lock);
        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            "important data\n"
        );

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[cfg(windows)]
    #[test]
    fn rejects_preexisting_lock_symlink_without_clobbering_target() {
        use std::os::windows::fs::symlink_file;

        let temp = std::env::temp_dir().join(format!(
            "fairtranslator-workspace-lock-symlink-test-{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        let victim = temp.join("victim.txt");
        std::fs::write(&victim, "important data\n").unwrap();
        if let Err(error) = symlink_file(&victim, temp.join(".fairtranslator-workspace.lock")) {
            let _ = std::fs::remove_dir_all(&temp);
            if error.kind() == std::io::ErrorKind::PermissionDenied
                || error.raw_os_error() == Some(1314)
            {
                eprintln!("skipping symlink assertion because this account cannot create symlinks");
                return;
            }
            panic!("failed to create test symlink: {error}");
        }

        assert!(WorkspaceRunLock::acquire(&temp).is_err());
        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            "important data\n"
        );

        let _ = std::fs::remove_dir_all(&temp);
    }
}
