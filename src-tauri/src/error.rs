use serde::Serialize;

/// Unified error type for all Tauri commands.
/// Uses `thiserror` for ergonomic Rust usage and `serde::Serialize` for IPC.
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("Error de E/S: {0}")]
    Io(String),

    #[error("Error de lectura: {0}")]
    Parse(String),

    #[error("Error del proyecto: {0}")]
    Project(String),

    #[error("Error de API: {0}")]
    Api(String),

    #[error("No encontrado: {0}")]
    NotFound(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Parse(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        let message = if e.is_timeout() {
            "La solicitud al proveedor agotó el tiempo de espera".to_string()
        } else if e.is_connect() {
            "No se pudo conectar con el proveedor".to_string()
        } else if e.is_decode() {
            "El proveedor devolvió una respuesta que no se pudo interpretar".to_string()
        } else if let Some(status) = e.status() {
            format!("La solicitud al proveedor falló con estado {status}")
        } else {
            "La solicitud al proveedor falló".to_string()
        };
        AppError::Api(message)
    }
}
