use std::{num::ParseIntError, string::FromUtf8Error};

use axum::{http::StatusCode, response::IntoResponse};
use thiserror::Error;
use tracing::error;

use crate::auth::AuthError;

#[derive(Debug, Error)]
pub enum KnawledgeError {
    #[error("IO: {0}")]
    IO(#[from] std::io::Error),

    #[error("UTF-8: {0}")]
    Utf8(#[from] FromUtf8Error),

    #[error("Parse int: {0}")]
    Parse(#[from] ParseIntError),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Inotify error: {0}")]
    Watcher(#[from] notify::Error),

    #[error("SQL: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Does not exist: {0}")]
    DoesNotExist(String),

    #[error("Invalid Directory: {0}")]
    InvalidDirectory(String),

    #[error("YAML error: {0}")]
    SerdeYaml(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Argon2 hash error: {0}")]
    A2Hash(argon2::Error),

    #[error("Argon2 validation error: {0}")]
    A2Validation(argon2::password_hash::Error),

    #[error("Http: {0}")]
    Http(#[from] axum::http::Error),

    #[error("Authentication: {0}")]
    Auth(#[from] AuthError),
}

impl From<argon2::Error> for KnawledgeError {
    fn from(value: argon2::Error) -> Self {
        Self::A2Hash(value)
    }
}

impl IntoResponse for KnawledgeError {
    fn into_response(self) -> axum::response::Response {
        error!("Error: {self}");

        use KnawledgeError as KE;

        match self {
            KE::NotFound(e) => (StatusCode::NOT_FOUND, e).into_response(),
            KE::IO(_)
            | KE::Parse(_)
            | KE::Utf8(_)
            | KE::Watcher(_)
            // This one can only occur on startup if an invalid hash is given
            | KE::A2Hash(_)
            | KE::Sqlx(_)
            | KE::SerdeYaml(_) | KE::Http(_)=> {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
            }
            KE::DoesNotExist(e) => (StatusCode::NOT_FOUND, e).into_response(),
            KE::InvalidDirectory(_) | KE::SerdeJson(_) => {
                (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()).into_response()
            }
            // Occurs on pw verification in handlers
            KE::A2Validation(e) => (StatusCode::UNAUTHORIZED, e.to_string()).into_response(),
            KE::Auth(e) => e.into_response()
        }
    }
}
