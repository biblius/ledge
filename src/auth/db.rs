use super::models::Session;
use crate::error::LedgeknawError;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct AuthDatabase {
    pool: sqlx::PgPool,
}

impl AuthDatabase {
    pub async fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert_session(
        &self,
        session_id: uuid::Uuid,
        expires: DateTime<Utc>,
    ) -> Result<Session, LedgeknawError> {
        Ok(sqlx::query_as!(
            Session,
            "INSERT INTO sessions VALUES ($1, $2) RETURNING id, expires, created_at, updated_at",
            session_id,
            expires,
        )
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn session_check(&self, session_id: uuid::Uuid) -> Result<bool, LedgeknawError> {
        let count = sqlx::query!(
            "SELECT COUNT(id) FROM sessions WHERE id = $1 AND expires > NOW()",
            session_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count.count.is_some_and(|count| count == 1))
    }
}
