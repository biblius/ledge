use self::{db::AuthDatabase, models::Session};
use crate::{config::AdminConfig, error::LedgeknawError};
use argon2::{password_hash::PasswordHashString, PasswordVerifier};
use axum::{http::StatusCode, response::IntoResponse};
use chrono::{Duration, Utc};
use cookie::{Cookie, SameSite};
use thiserror::Error;

pub mod db;
pub mod models;

#[derive(Debug)]
pub struct Auth {
    cookie_domain: String,
    pw_hash: PasswordHashString,
    db: AuthDatabase,
}

impl Auth {
    pub fn new(
        db: AuthDatabase,
        AdminConfig {
            cookie_domain,
            pw_hash,
        }: AdminConfig,
    ) -> Self {
        Self {
            cookie_domain,
            pw_hash: PasswordHashString::new(&pw_hash).expect("error in auth configuration"),
            db,
        }
    }

    pub fn verify_password(&self, password: &str) -> bool {
        argon2::Argon2::default()
            .verify_password(password.as_bytes(), &self.pw_hash.password_hash())
            .is_ok()
    }

    pub async fn session_check(&self, session_id: uuid::Uuid) -> Result<bool, LedgeknawError> {
        self.db.session_check(session_id).await
    }

    pub async fn create_session(&self) -> Result<Session, LedgeknawError> {
        let id = uuid::Uuid::new_v4();
        let expires = Utc::now().to_utc() + Duration::hours(1);
        self.db.insert_session(id, expires).await
    }

    pub fn create_session_cookie(&self, session_id: uuid::Uuid) -> Cookie<'_> {
        let mut cookie = axum_extra::extract::cookie::Cookie::new("SID", session_id.to_string());
        cookie.set_secure(true);
        cookie.set_http_only(true);
        cookie.set_same_site(SameSite::Strict);
        cookie.set_domain(&self.cookie_domain);
        cookie.set_path("/admin");
        cookie.set_max_age(cookie::time::Duration::hours(1));
        cookie
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Session required for requested resource")]
    NoSession,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::NoSession => (StatusCode::UNAUTHORIZED, "No session").into_response(),
        }
    }
}
