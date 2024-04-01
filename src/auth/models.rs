use chrono::{DateTime, Utc};

/// Session model
#[derive(Debug)]
pub struct Session {
    pub id: uuid::Uuid,
    pub expires: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
