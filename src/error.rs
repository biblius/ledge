use std::{num::ParseIntError, string::FromUtf8Error};

use axum::{http::StatusCode, response::IntoResponse};
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum KnawledgeError {
    #[error("{0}")]
    IO(#[from] std::io::Error),

    #[error("{0}")]
    Utf8(#[from] FromUtf8Error),

    #[error("{0}")]
    Parse(#[from] ParseIntError),

    #[error("{0}")]
    MiniJinja(#[from] minijinja::Error),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Watcher(#[from] notify::Error),

    #[error("{0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("{0}")]
    DoesNotExist(String),

    #[error("{0}")]
    InvalidDirectory(String),
}

impl IntoResponse for KnawledgeError {
    fn into_response(self) -> axum::response::Response {
        error!("Error: {self}");
        match self {
            KnawledgeError::MiniJinja(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
            KnawledgeError::IO(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
            KnawledgeError::Parse(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
            KnawledgeError::Utf8(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            KnawledgeError::NotFound(e) => (StatusCode::NOT_FOUND, e).into_response(),
            KnawledgeError::Watcher(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
            KnawledgeError::DoesNotExist(e) => (StatusCode::NOT_FOUND, e).into_response(),
            e => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }
}
