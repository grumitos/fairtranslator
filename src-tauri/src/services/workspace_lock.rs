use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

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
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        #[cfg(windows)]
        {
            options.create(true).truncate(true).share_mode(0);
        }
        #[cfg(not(windows))]
        {
            options.create_new(true);
        }

        let mut file = options.open(&path).map_err(|error| {
            format!(
                "El workspace '{}' ya está en uso por otro proceso o no se puede bloquear: {error}",
                workspace.display()
            )
        })?;
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

        Ok(Self {
            path,
            file: Some(file),
        })
    }
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
}
