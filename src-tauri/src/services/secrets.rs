use crate::error::AppError;

const SERVICE_NAME: &str = "com.fairtranslator.desktop";

pub fn load_secret(key: &str) -> Result<Option<String>, AppError> {
    platform::load_secret(key)
}

pub fn save_secret(key: &str, value: &str) -> Result<(), AppError> {
    platform::save_secret(key, value)
}

pub fn delete_secret(key: &str) -> Result<(), AppError> {
    platform::delete_secret(key)
}

#[cfg(target_os = "windows")]
mod platform {
    use keyring::{Entry, Error as KeyringError};

    use super::SERVICE_NAME;
    use crate::error::AppError;

    fn entry(key: &str) -> Result<Entry, AppError> {
        Entry::new(SERVICE_NAME, key)
            .map_err(|error| AppError::Project(format!("Failed to open secure key store: {error}")))
    }

    pub fn load_secret(key: &str) -> Result<Option<String>, AppError> {
        match entry(key)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(AppError::Project(format!(
                "Failed to read secure key store: {error}"
            ))),
        }
    }

    pub fn save_secret(key: &str, value: &str) -> Result<(), AppError> {
        entry(key)?
            .set_password(value)
            .map_err(|error| AppError::Project(format!("Failed to save secure key store: {error}")))
    }

    pub fn delete_secret(key: &str) -> Result<(), AppError> {
        match entry(key)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(AppError::Project(format!(
                "Failed to delete secure key store entry: {error}"
            ))),
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use crate::error::AppError;

    pub fn load_secret(_key: &str) -> Result<Option<String>, AppError> {
        Err(AppError::Project(
            "Secure key storage is not supported on this platform".to_string(),
        ))
    }

    pub fn save_secret(_key: &str, _value: &str) -> Result<(), AppError> {
        Err(AppError::Project(
            "Secure key storage is not supported on this platform".to_string(),
        ))
    }

    pub fn delete_secret(_key: &str) -> Result<(), AppError> {
        Err(AppError::Project(
            "Secure key storage is not supported on this platform".to_string(),
        ))
    }
}
